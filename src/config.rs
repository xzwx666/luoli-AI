use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub security: SecurityConfig,
    pub monitor: MonitorConfig,
    pub mcp: McpConfig,
    pub api: ApiConfig,
    pub ai: crate::ai_config::AiConfigManager,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub default_mode: String,
    pub command_allowlist: Vec<String>,
    pub command_denylist: Vec<String>,
    pub tool_allowlist: Vec<String>,
    pub tool_denylist: Vec<String>,
    pub api_allowlist: Vec<String>,
    pub api_denylist: Vec<String>,
    pub max_command_length: usize,
    pub forbidden_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    pub enabled: bool,
    pub log_file: PathBuf,
    pub max_log_size: usize,
    pub log_rotation: usize,
    pub capture_output: bool,
    pub capture_env: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub enabled: bool,
    pub tools: Vec<McpTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub enabled: bool,
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub enabled: bool,
    pub endpoints: Vec<ApiEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEndpoint {
    pub name: String,
    pub url: String,
    pub method: String,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_timeout() -> u64 {
    30
}

fn default_true() -> bool {
    true
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        
        if !config_path.exists() {
            println!("配置文件不存在，创建默认配置...");
            return Self::init_default();
        }
        
        let content = std::fs::read_to_string(&config_path)?;
        
        // 尝试解析配置，如果缺少字段则使用默认值
        let mut config: AppConfig = match toml::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                println!("配置文件格式不兼容，重新创建默认配置...");
                println!("错误: {}", e);
                return Self::init_default();
            }
        };
        
        // 加载 AI 配置（从单独的 JSON 文件）
        config.ai = crate::ai_config::AiConfigManager::load()?;
        
        Ok(config)
    }
    
    pub fn init_default() -> Result<Self> {
        let config = Self::default();
        let config_path = Self::config_path()?;
        
        // 确保配置目录存在
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = toml::to_string_pretty(&config)?;
        std::fs::write(&config_path, content)?;
        
        Ok(config)
    }
    
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }
    
    fn config_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("无法获取用户主目录"))?;
        Ok(home.join(".luoli").join("config.toml"))
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        
        Self {
            security: SecurityConfig {
                default_mode: "normal".to_string(),
                command_allowlist: vec![
                    "^ls$".to_string(),
                    "^ls\\s+".to_string(),
                    "^pwd$".to_string(),
                    "^cd\\s+".to_string(),
                    "^cat\\s+".to_string(),
                    "^echo\\s+".to_string(),
                    "^mkdir\\s+".to_string(),
                    "^touch\\s+".to_string(),
                    "^cp\\s+".to_string(),
                    "^mv\\s+".to_string(),
                    "^rm\\s+".to_string(),
                    "^git\\s+".to_string(),
                    "^python\\s+".to_string(),
                    "^python3\\s+".to_string(),
                    "^cargo\\s+".to_string(),
                    "^rustc\\s+".to_string(),
                ],
                command_denylist: vec![
                    "rm\\s+-rf\\s+/".to_string(),
                    ">/dev/sda".to_string(),
                    "mkfs\\.".to_string(),
                    "dd\\s+if=.*/dev/zero.*of=/dev/sda".to_string(),
                    ":(){ :|:& };:".to_string(),
                ],
                tool_allowlist: vec![
                    "file_read".to_string(),
                    "file_write".to_string(),
                    "shell_execute".to_string(),
                    "web_search".to_string(),
                ],
                tool_denylist: vec![],
                api_allowlist: vec![
                    "https://api.github.com".to_string(),
                ],
                api_denylist: vec![],
                max_command_length: 4096,
                forbidden_patterns: vec![
                    "password".to_string(),
                    "secret".to_string(),
                    "token".to_string(),
                    "api_key".to_string(),
                ],
            },
            monitor: MonitorConfig {
                enabled: true,
                log_file: home.join(".luoli").join("logs").join("activity.log"),
                max_log_size: 10 * 1024 * 1024, // 10MB
                log_rotation: 5,
                capture_output: true,
                capture_env: false,
            },
            mcp: McpConfig {
                enabled: true,
                tools: vec![
                    McpTool {
                        name: "filesystem".to_string(),
                        enabled: true,
                        config: serde_json::json!({}),
                    },
                    McpTool {
                        name: "shell".to_string(),
                        enabled: true,
                        config: serde_json::json!({}),
                    },
                ],
            },
            api: ApiConfig {
                enabled: true,
                endpoints: vec![],
            },
            ai: crate::ai_config::AiConfigManager::new(),
        }
    }
}
