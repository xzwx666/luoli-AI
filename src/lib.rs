use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::sync::Arc;
use tokio::runtime::Runtime;

pub mod config;
pub mod security;
pub mod monitor;
pub mod executor;
pub mod mcp;
pub mod api;

use security::{SecurityManager, ExecutionMode};
use monitor::ActivityMonitor;
use executor::CommandExecutor;
use mcp::McpToolRegistry;

/// Python 绑定的终端助手
#[pyclass]
struct LuoliAssistant {
    runtime: Runtime,
    security: Arc<tokio::sync::RwLock<SecurityManager>>,
    monitor: Arc<ActivityMonitor>,
    executor: Arc<CommandExecutor>,
    mcp_registry: Arc<tokio::sync::RwLock<McpToolRegistry>>,
}

#[pymethods]
impl LuoliAssistant {
    /// 创建新的助手实例
    #[new]
    fn new() -> PyResult<Self> {
        let runtime = Runtime::new()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        
        let config = config::AppConfig::load()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        
        let security = Arc::new(tokio::sync::RwLock::new(
            SecurityManager::new(config.security)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?
        ));
        
        let monitor = Arc::new(
            ActivityMonitor::new(config.monitor)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?
        );
        
        let executor = Arc::new(CommandExecutor::new(security.clone(), monitor.clone()));
        
        let mcp_registry = Arc::new(tokio::sync::RwLock::new(
            McpToolRegistry::new(security.clone())
        ));
        
        Ok(Self {
            runtime,
            security,
            monitor,
            executor,
            mcp_registry,
        })
    }

    /// 执行命令
    fn execute(&self, command: &str) -> PyResult<String> {
        self.runtime.block_on(async {
            self.executor.execute(command).await
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    /// 设置运行模式
    fn set_mode(&self, mode: &str) -> PyResult<()> {
        let mode = match mode {
            "strict" | "严格" => ExecutionMode::Strict,
            "normal" | "普通" => ExecutionMode::Normal,
            _ => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "无效的模式，请使用 'strict' 或 'normal'"
            )),
        };
        
        self.runtime.block_on(async {
            self.security.write().await.set_mode(mode);
        });
        
        Ok(())
    }

    /// 获取当前模式
    fn get_mode(&self) -> PyResult<String> {
        let mode = self.runtime.block_on(async {
            self.security.read().await.mode()
        });
        
        Ok(match mode {
            ExecutionMode::Strict => "strict".to_string(),
            ExecutionMode::Normal => "normal".to_string(),
        })
    }

    /// 添加命令白名单
    fn add_command_allowlist(&self, pattern: &str) -> PyResult<()> {
        self.runtime.block_on(async {
            self.security.write().await.add_command_allowlist(pattern)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    /// 添加命令黑名单
    fn add_command_denylist(&self, pattern: &str) -> PyResult<()> {
        self.runtime.block_on(async {
            self.security.write().await.add_command_denylist(pattern)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    /// 执行 MCP 工具
    fn execute_tool(&self, name: &str, params: &PyDict) -> PyResult<PyObject> {
        let params: serde_json::Value = pydict_to_json(params)?;
        
        let result = self.runtime.block_on(async {
            self.mcp_registry.read().await.execute_tool(name, params).await
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })?;
        
        Python::with_gil(|py| {
            json_to_pyobject(py, result)
        })
    }

    /// 列出 MCP 工具
    fn list_tools(&self) -> PyResult<Vec<(String, String)>> {
        let tools = self.runtime.block_on(async {
            self.mcp_registry.read().await.list_tools()
        });
        
        Ok(tools.into_iter()
            .map(|(name, desc)| (name.to_string(), desc.to_string()))
            .collect())
    }

    /// 获取最近的日志
    fn get_recent_logs(&self, limit: usize) -> PyResult<Vec<PyObject>> {
        let records = self.runtime.block_on(async {
            self.monitor.get_recent_records(limit).await
        });
        
        Python::with_gil(|py| {
            records.into_iter()
                .map(|r| {
                    let dict = PyDict::new(py);
                    dict.set_item("id", r.id)?;
                    dict.set_item("timestamp", r.timestamp.to_rfc3339())?;
                    dict.set_item("type", format!("{:?}", r.activity_type))?;
                    dict.set_item("command", r.command)?;
                    dict.set_item("status", format!("{:?}", r.status))?;
                    Ok(dict.to_object(py))
                })
                .collect::<PyResult<Vec<_>>>()
        })
    }
}

/// 将 PyDict 转换为 JSON
fn pydict_to_json(dict: &PyDict) -> PyResult<serde_json::Value> {
    let json_str = Python::with_gil(|py| {
        let json_module = py.import("json")?;
        json_module.getattr("dumps")?.call1((dict,))?.extract::<String>()
    })?;
    
    serde_json::from_str(&json_str)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))
}

/// 将 JSON 转换为 PyObject
fn json_to_pyobject(py: Python, value: serde_json::Value) -> PyResult<PyObject> {
    match value {
        serde_json::Value::Null => Ok(py.None()),
        serde_json::Value::Bool(b) => Ok(b.to_object(py)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.to_object(py))
            } else if let Some(f) = n.as_f64() {
                Ok(f.to_object(py))
            } else {
                Ok(n.to_string().to_object(py))
            }
        }
        serde_json::Value::String(s) => Ok(s.to_object(py)),
        serde_json::Value::Array(arr) => {
            let list = PyList::empty(py);
            for item in arr {
                list.append(json_to_pyobject(py, item)?)?;
            }
            Ok(list.to_object(py))
        }
        serde_json::Value::Object(obj) => {
            let dict = PyDict::new(py);
            for (k, v) in obj {
                dict.set_item(k, json_to_pyobject(py, v)?)?;
            }
            Ok(dict.to_object(py))
        }
    }
}

/// Python 模块定义
#[pymodule]
fn luoli_assistant(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<LuoliAssistant>()?;
    Ok(())
}
