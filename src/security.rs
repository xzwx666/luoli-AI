use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use anyhow::{Result, anyhow};
use log::{info, warn};

use crate::config::SecurityConfig;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// 严格模式：只允许白名单内的命令、工具和API
    Strict,
    /// 普通模式：允许执行大部分命令，但会检查黑名单
    Normal,
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionMode::Strict => write!(f, "严格模式"),
            ExecutionMode::Normal => write!(f, "普通模式"),
        }
    }
}

pub struct SecurityManager {
    mode: ExecutionMode,
    config: SecurityConfig,
    command_allow_patterns: Vec<Regex>,
    command_deny_patterns: Vec<Regex>,
    tool_allowlist: HashSet<String>,
    tool_denylist: HashSet<String>,
    api_allowlist: HashSet<String>,
    api_denylist: HashSet<String>,
}

impl SecurityManager {
    pub fn new(config: SecurityConfig) -> Result<Self> {
        let mut manager = Self {
            mode: match config.default_mode.as_str() {
                "strict" => ExecutionMode::Strict,
                _ => ExecutionMode::Normal,
            },
            config: config.clone(),
            command_allow_patterns: Vec::new(),
            command_deny_patterns: Vec::new(),
            tool_allowlist: HashSet::new(),
            tool_denylist: HashSet::new(),
            api_allowlist: HashSet::new(),
            api_denylist: HashSet::new(),
        };

        // 编译命令白名单正则
        for pattern in &config.command_allowlist {
            match Regex::new(pattern) {
                Ok(re) => manager.command_allow_patterns.push(re),
                Err(e) => warn!("无效的白名单正则 '{}': {}", pattern, e),
            }
        }

        // 编译命令黑名单正则
        for pattern in &config.command_denylist {
            match Regex::new(pattern) {
                Ok(re) => manager.command_deny_patterns.push(re),
                Err(e) => warn!("无效的黑名单正则 '{}': {}", pattern, e),
            }
        }

        // 加载工具黑白名单
        for tool in &config.tool_allowlist {
            manager.tool_allowlist.insert(tool.clone());
        }
        for tool in &config.tool_denylist {
            manager.tool_denylist.insert(tool.clone());
        }

        // 加载API黑白名单
        for api in &config.api_allowlist {
            manager.api_allowlist.insert(api.clone());
        }
        for api in &config.api_denylist {
            manager.api_denylist.insert(api.clone());
        }

        info!("安全管控模块初始化完成，当前模式: {}", manager.mode);
        Ok(manager)
    }

    /// 检查命令是否允许执行
    pub fn check_command(&self, command: &str) -> Result<()> {
        // 检查命令长度
        if command.len() > self.config.max_command_length {
            return Err(anyhow!(
                "命令长度超过限制 ({} > {})",
                command.len(),
                self.config.max_command_length
            ));
        }

        // 检查危险模式
        for pattern in &self.config.forbidden_patterns {
            if command.to_lowercase().contains(pattern) {
                return Err(anyhow!(
                    "命令包含禁止的模式: '{}'",
                    pattern
                ));
            }
        }

        // 检查黑名单
        for pattern in &self.command_deny_patterns {
            if pattern.is_match(command) {
                return Err(anyhow!(
                    "命令匹配黑名单规则: '{}'",
                    pattern.as_str()
                ));
            }
        }

        // 严格模式下检查白名单
        if self.mode == ExecutionMode::Strict {
            let allowed = self.command_allow_patterns.iter().any(|p| p.is_match(command));
            if !allowed {
                return Err(anyhow!(
                    "严格模式：命令 '{}' 不在白名单内，拒绝执行",
                    command
                ));
            }
        }

        Ok(())
    }

    /// 检查工具是否允许使用
    pub fn check_tool(&self, tool_name: &str) -> Result<()> {
        // 检查黑名单
        if self.tool_denylist.contains(tool_name) {
            return Err(anyhow!(
                "工具 '{}' 在黑名单中，禁止使用",
                tool_name
            ));
        }

        // 严格模式下检查白名单
        if self.mode == ExecutionMode::Strict {
            if !self.tool_allowlist.contains(tool_name) {
                return Err(anyhow!(
                    "严格模式：工具 '{}' 不在白名单内，拒绝使用",
                    tool_name
                ));
            }
        }

        Ok(())
    }

    /// 检查API是否允许访问
    pub fn check_api(&self, api_url: &str) -> Result<()> {
        // 检查黑名单
        for denied in &self.api_denylist {
            if api_url.contains(denied) {
                return Err(anyhow!(
                    "API '{}' 匹配黑名单规则 '{}'，禁止访问",
                    api_url,
                    denied
                ));
            }
        }

        // 严格模式下检查白名单
        if self.mode == ExecutionMode::Strict {
            let allowed = self.api_allowlist.iter().any(|allowed| api_url.contains(allowed));
            if !allowed {
                return Err(anyhow!(
                    "严格模式：API '{}' 不在白名单内，拒绝访问",
                    api_url
                ));
            }
        }

        Ok(())
    }

    /// 设置运行模式
    pub fn set_mode(&mut self, mode: ExecutionMode) {
        self.mode = mode;
        info!("运行模式已切换为: {}", mode);
    }

    /// 获取当前模式
    pub fn mode(&self) -> ExecutionMode {
        self.mode
    }

    /// 添加命令白名单
    pub fn add_command_allowlist(&mut self, pattern: &str) -> Result<()> {
        let re = Regex::new(pattern)?;
        self.command_allow_patterns.push(re);
        self.config.command_allowlist.push(pattern.to_string());
        info!("添加命令白名单: {}", pattern);
        Ok(())
    }

    /// 添加命令黑名单
    pub fn add_command_denylist(&mut self, pattern: &str) -> Result<()> {
        let re = Regex::new(pattern)?;
        self.command_deny_patterns.push(re);
        self.config.command_denylist.push(pattern.to_string());
        info!("添加命令黑名单: {}", pattern);
        Ok(())
    }

    /// 添加工具白名单
    pub fn add_tool_allowlist(&mut self, name: &str) -> Result<()> {
        self.tool_allowlist.insert(name.to_string());
        self.config.tool_allowlist.push(name.to_string());
        info!("添加工具白名单: {}", name);
        Ok(())
    }

    /// 添加工具黑名单
    pub fn add_tool_denylist(&mut self, name: &str) -> Result<()> {
        self.tool_denylist.insert(name.to_string());
        self.config.tool_denylist.push(name.to_string());
        info!("添加工具黑名单: {}", name);
        Ok(())
    }

    /// 列出所有规则
    pub fn list_rules(&self) {
        println!("\n=== 安全规则配置 ===");
        println!("当前模式: {}", self.mode);
        
        println!("\n命令白名单:");
        for pattern in &self.config.command_allowlist {
            println!("  + {}", pattern);
        }
        
        println!("\n命令黑名单:");
        for pattern in &self.config.command_denylist {
            println!("  - {}", pattern);
        }
        
        println!("\n工具白名单:");
        for tool in &self.config.tool_allowlist {
            println!("  + {}", tool);
        }
        
        println!("\n工具黑名单:");
        for tool in &self.config.tool_denylist {
            println!("  - {}", tool);
        }
        
        println!("\nAPI白名单:");
        for api in &self.config.api_allowlist {
            println!("  + {}", api);
        }
        
        println!("\nAPI黑名单:");
        for api in &self.config.api_denylist {
            println!("  - {}", api);
        }
        
        println!("\n禁止模式:");
        for pattern in &self.config.forbidden_patterns {
            println!("  ! {}", pattern);
        }
    }

    /// 保存配置
    pub fn save_config(&self) -> Result<()> {
        crate::config::AppConfig::save(&crate::config::AppConfig {
            security: self.config.clone(),
            monitor: crate::config::MonitorConfig {
                enabled: true,
                log_file: std::path::PathBuf::from("~/.luoli/logs/activity.log"),
                max_log_size: 10 * 1024 * 1024,
                log_rotation: 5,
                capture_output: true,
                capture_env: false,
            },
            mcp: crate::config::McpConfig {
                enabled: true,
                tools: vec![],
            },
            api: crate::config::ApiConfig {
                enabled: true,
                endpoints: vec![],
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> SecurityConfig {
        SecurityConfig {
            default_mode: "normal".to_string(),
            command_allowlist: vec![
                "^ls\\s*".to_string(),
                "^cat\\s+".to_string(),
            ],
            command_denylist: vec![
                "rm\\s+-rf\\s+/".to_string(),
            ],
            tool_allowlist: vec!["file_read".to_string()],
            tool_denylist: vec!["dangerous_tool".to_string()],
            api_allowlist: vec!["https://api.github.com".to_string()],
            api_denylist: vec![],
            max_command_length: 4096,
            forbidden_patterns: vec!["password".to_string()],
        }
    }

    #[test]
    fn test_check_command_normal_mode() {
        let config = create_test_config();
        let manager = SecurityManager::new(config).unwrap();

        // 正常命令应该通过
        assert!(manager.check_command("ls -la").is_ok());
        
        // 黑名单命令应该被拒绝
        assert!(manager.check_command("rm -rf /").is_err());
        
        // 包含禁止模式的命令应该被拒绝
        assert!(manager.check_command("echo password").is_err());
    }

    #[test]
    fn test_check_command_strict_mode() {
        let config = create_test_config();
        let mut manager = SecurityManager::new(config).unwrap();
        manager.set_mode(ExecutionMode::Strict);

        // 白名单内的命令应该通过
        assert!(manager.check_command("ls -la").is_ok());
        assert!(manager.check_command("cat file.txt").is_ok());
        
        // 不在白名单的命令应该被拒绝
        assert!(manager.check_command("echo hello").is_err());
    }
}
