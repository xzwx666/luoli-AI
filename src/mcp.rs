use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::{Result, Context};
use log::{info, warn, error};

use crate::config::McpConfig;
use crate::security::SecurityManager;

/// MCP 工具接口
#[async_trait::async_trait]
pub trait McpTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn execute(&self, params: serde_json::Value) -> Result<serde_json::Value>;
}

/// MCP 工具注册表
pub struct McpToolRegistry {
    tools: HashMap<String, Arc<dyn McpTool>>,
    security: Arc<RwLock<SecurityManager>>,
}

impl McpToolRegistry {
    pub fn new(security: Arc<RwLock<SecurityManager>>) -> Self {
        Self {
            tools: HashMap::new(),
            security,
        }
    }

    /// 注册工具
    pub fn register_tool(&mut self, tool: Arc<dyn McpTool>) -> Result<()> {
        let name = tool.name().to_string();
        info!("注册 MCP 工具: {}", name);
        self.tools.insert(name, tool);
        Ok(())
    }

    /// 执行工具
    pub async fn execute_tool(&self, name: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        // 安全检查
        {
            let security = self.security.read().await;
            security.check_tool(name)?;
        }

        let tool = self.tools.get(name)
            .ok_or_else(|| anyhow::anyhow!("工具 '{}' 未找到", name))?;

        info!("执行 MCP 工具: {}", name);
        let result = tool.execute(params).await;
        
        match &result {
            Ok(_) => info!("工具 '{}' 执行成功", name),
            Err(e) => error!("工具 '{}' 执行失败: {}", name, e),
        }
        
        result
    }

    /// 列出所有可用工具
    pub fn list_tools(&self) -> Vec<(&str, &str)> {
        self.tools
            .iter()
            .map(|(name, tool)| (name.as_str(), tool.description()))
            .collect()
    }

    /// 检查工具是否存在
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}

/// 文件系统工具
pub struct FileSystemTool;

#[async_trait::async_trait]
impl McpTool for FileSystemTool {
    fn name(&self) -> &str {
        "filesystem"
    }

    fn description(&self) -> &str {
        "文件系统操作工具，支持读写文件、列出目录等"
    }

    async fn execute(&self, params: serde_json::Value) -> Result<serde_json::Value> {
        let action = params.get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("缺少 action 参数"))?;

        match action {
            "read" => {
                let path = params.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("缺少 path 参数"))?;
                
                let content = tokio::fs::read_to_string(path).await
                    .with_context(|| format!("读取文件失败: {}", path))?;
                
                Ok(serde_json::json!({
                    "success": true,
                    "content": content,
                }))
            }
            "write" => {
                let path = params.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("缺少 path 参数"))?;
                
                let content = params.get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("缺少 content 参数"))?;
                
                tokio::fs::write(path, content).await
                    .with_context(|| format!("写入文件失败: {}", path))?;
                
                Ok(serde_json::json!({
                    "success": true,
                    "message": "文件写入成功",
                }))
            }
            "list" => {
                let path = params.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("缺少 path 参数"))?;
                
                let mut entries = Vec::new();
                let mut dir = tokio::fs::read_dir(path).await
                    .with_context(|| format!("读取目录失败: {}", path))?;
                
                while let Some(entry) = dir.next_entry().await? {
                    let metadata = entry.metadata().await?;
                    entries.push(serde_json::json!({
                        "name": entry.file_name().to_string_lossy().to_string(),
                        "is_file": metadata.is_file(),
                        "is_dir": metadata.is_dir(),
                        "size": metadata.len(),
                    }));
                }
                
                Ok(serde_json::json!({
                    "success": true,
                    "entries": entries,
                }))
            }
            _ => Err(anyhow::anyhow!("未知的 action: {}", action)),
        }
    }
}

/// Shell 执行工具
pub struct ShellTool {
    security: Arc<RwLock<SecurityManager>>,
}

impl ShellTool {
    pub fn new(security: Arc<RwLock<SecurityManager>>) -> Self {
        Self { security }
    }
}

#[async_trait::async_trait]
impl McpTool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "Shell 命令执行工具"
    }

    async fn execute(&self, params: serde_json::Value) -> Result<serde_json::Value> {
        let command = params.get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("缺少 command 参数"))?;

        // 安全检查
        {
            let security = self.security.read().await;
            security.check_command(command)?;
        }

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .await
            .with_context(|| format!("执行命令失败: {}", command))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(serde_json::json!({
            "success": output.status.success(),
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": output.status.code(),
        }))
    }
}

/// Web 搜索工具
pub struct WebSearchTool;

#[async_trait::async_trait]
impl McpTool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Web 搜索工具"
    }

    async fn execute(&self, params: serde_json::Value) -> Result<serde_json::Value> {
        let query = params.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("缺少 query 参数"))?;

        // 这里可以实现实际的搜索逻辑
        // 例如调用搜索引擎 API
        
        Ok(serde_json::json!({
            "success": true,
            "query": query,
            "results": [],
            "message": "搜索功能待实现",
        }))
    }
}

/// 初始化默认工具
pub fn init_default_tools(
    registry: &mut McpToolRegistry,
    security: Arc<RwLock<SecurityManager>>,
) -> Result<()> {
    // 注册文件系统工具
    registry.register_tool(Arc::new(FileSystemTool))?;
    
    // 注册 Shell 工具
    registry.register_tool(Arc::new(ShellTool::new(security)))?;
    
    // 注册 Web 搜索工具
    registry.register_tool(Arc::new(WebSearchTool))?;
    
    info!("默认 MCP 工具初始化完成");
    Ok(())
}
