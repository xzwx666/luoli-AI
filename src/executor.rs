use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::RwLock;
use anyhow::{Result, Context};
use log::{info, error, debug};

use crate::security::SecurityManager;
use crate::monitor::{ActivityMonitor, ActivityRecord, ActivityType, ActivityStatus};

pub struct CommandExecutor {
    security: Arc<RwLock<SecurityManager>>,
    monitor: Arc<ActivityMonitor>,
}

impl CommandExecutor {
    pub fn new(
        security: Arc<RwLock<SecurityManager>>,
        monitor: Arc<ActivityMonitor>,
    ) -> Self {
        Self { security, monitor }
    }

    /// 执行命令
    pub async fn execute(&self, command_str: &str) -> Result<String> {
        let start_time = Instant::now();
        
        // 解析命令
        let (cmd, args) = self.parse_command(command_str);
        
        // 安全检查
        {
            let security = self.security.read().await;
            if let Err(e) = security.check_command(command_str) {
                // 记录安全违规
                let record = ActivityRecord::new(
                    ActivityType::SecurityViolation,
                    whoami::username(),
                    cmd.clone(),
                    args.clone(),
                )
                .with_status(ActivityStatus::Blocked(e.to_string()))
                .with_duration(start_time.elapsed().as_millis() as u64);
                
                self.monitor.log_activity(record).await?;
                
                return Err(e);
            }
        }
        
        // 创建活动记录
        let mut record = ActivityRecord::new(
            ActivityType::CommandExecution,
            whoami::username(),
            cmd.clone(),
            args.clone(),
        );
        
        debug!("执行命令: {}", command_str);
        
        // 执行命令
        let result = self.run_command(&cmd, &args).await;
        
        // 更新记录
        let duration = start_time.elapsed().as_millis() as u64;
        record = record.with_duration(duration);
        
        match &result {
            Ok(output) => {
                record = record
                    .with_status(ActivityStatus::Success)
                    .with_output(output.clone());
                info!("命令执行成功: {} ({}ms)", cmd, duration);
            }
            Err(e) => {
                record = record
                    .with_status(ActivityStatus::Failed(e.to_string()));
                error!("命令执行失败: {} - {}", cmd, e);
            }
        }
        
        // 记录活动
        self.monitor.log_activity(record).await?;
        
        result
    }

    /// 解析命令字符串
    fn parse_command(&self, command_str: &str) -> (String, Vec<String>) {
        let parts: Vec<&str> = command_str.split_whitespace().collect();
        if parts.is_empty() {
            return (String::new(), Vec::new());
        }
        
        let cmd = parts[0].to_string();
        let args = parts[1..].iter().map(|s| s.to_string()).collect();
        
        (cmd, args)
    }

    /// 运行系统命令
    async fn run_command(&self, cmd: &str, args: &[String]) -> Result<String> {
        let mut command = Command::new(cmd);
        command.args(args);
        
        // 设置标准输入输出
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        
        // 执行命令
        let mut child = command.spawn()
            .with_context(|| format!("无法启动命令: {}", cmd))?;
        
        // 读取输出
        let mut stdout = String::new();
        let mut stderr = String::new();
        
        if let Some(mut out) = child.stdout.take() {
            out.read_to_string(&mut stdout).await?;
        }
        
        if let Some(mut err) = child.stderr.take() {
            err.read_to_string(&mut stderr).await?;
        }
        
        // 等待命令完成
        let status = child.wait().await?;
        
        if !status.success() {
            let error_msg = if !stderr.is_empty() {
                stderr
            } else {
                format!("命令退出码: {}", status.code().unwrap_or(-1))
            };
            return Err(anyhow::anyhow!("命令执行失败: {}", error_msg));
        }
        
        Ok(stdout)
    }

    /// 执行管道命令
    pub async fn execute_pipeline(&self, pipeline: &str) -> Result<String> {
        let commands: Vec<&str> = pipeline.split('|').map(|s| s.trim()).collect();
        
        if commands.is_empty() {
            return Err(anyhow::anyhow!("空管道命令"));
        }
        
        let mut last_output = String::new();
        
        for (i, cmd_str) in commands.iter().enumerate() {
            let (cmd, args) = self.parse_command(cmd_str);
            
            // 安全检查
            {
                let security = self.security.read().await;
                security.check_command(cmd_str)?;
            }
            
            let mut command = Command::new(&cmd);
            command.args(&args);
            
            // 如果不是第一个命令，将上一个输出作为输入
            if i > 0 && !last_output.is_empty() {
                command.stdin(Stdio::piped());
            }
            
            command.stdout(Stdio::piped());
            command.stderr(Stdio::piped());
            
            let mut child = command.spawn()
                .with_context(|| format!("无法启动命令: {}", cmd))?;
            
            // 如果有输入数据，写入stdin
            if i > 0 && !last_output.is_empty() {
                if let Some(mut stdin) = child.stdin.take() {
                    stdin.write_all(last_output.as_bytes()).await?;
                }
            }
            
            // 读取输出
            last_output.clear();
            if let Some(mut out) = child.stdout.take() {
                out.read_to_string(&mut last_output).await?;
            }
            
            let status = child.wait().await?;
            if !status.success() {
                return Err(anyhow::anyhow!("管道命令 '{}' 执行失败", cmd));
            }
        }
        
        Ok(last_output)
    }

    /// 执行带超时的命令
    pub async fn execute_with_timeout(
        &self,
        command_str: &str,
        timeout_secs: u64,
    ) -> Result<String> {
        let result = tokio::time::timeout(
            tokio::time::Duration::from_secs(timeout_secs),
            self.execute(command_str)
        ).await;
        
        match result {
            Ok(Ok(output)) => Ok(output),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(anyhow::anyhow!("命令执行超时 ({}秒)", timeout_secs)),
        }
    }

    /// 执行后台命令
    pub async fn execute_background(&self, command_str: &str) -> Result<u32> {
        let (cmd, args) = self.parse_command(command_str);
        
        // 安全检查
        {
            let security = self.security.read().await;
            security.check_command(command_str)?;
        }
        
        let mut command = Command::new(&cmd);
        command.args(&args);
        
        // 分离进程
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        
        let child = command.spawn()
            .with_context(|| format!("无法启动后台命令: {}", cmd))?;
        
        let pid = child.id().unwrap_or(0);
        info!("后台进程已启动: PID {}", pid);
        
        // 记录活动
        let record = ActivityRecord::new(
            ActivityType::CommandExecution,
            whoami::username(),
            cmd,
            args,
        )
        .with_status(ActivityStatus::Success)
        .with_metadata(serde_json::json!({
            "pid": pid,
            "background": true,
        }));
        
        self.monitor.log_activity(record).await?;
        
        Ok(pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{SecurityConfig, MonitorConfig};
    use std::path::PathBuf;

    async fn create_test_executor() -> CommandExecutor {
        let security_config = SecurityConfig {
            default_mode: "normal".to_string(),
            command_allowlist: vec![],
            command_denylist: vec!["rm\\s+-rf\\s+/".to_string()],
            tool_allowlist: vec![],
            tool_denylist: vec![],
            api_allowlist: vec![],
            api_denylist: vec![],
            max_command_length: 4096,
            forbidden_patterns: vec![],
        };
        
        let monitor_config = MonitorConfig {
            enabled: true,
            log_file: PathBuf::from("/tmp/test.log"),
            max_log_size: 1024 * 1024,
            log_rotation: 3,
            capture_output: true,
            capture_env: false,
        };
        
        let security = Arc::new(RwLock::new(
            SecurityManager::new(security_config).unwrap()
        ));
        let monitor = Arc::new(
            ActivityMonitor::new(monitor_config).unwrap()
        );
        
        CommandExecutor::new(security, monitor)
    }

    #[tokio::test]
    async fn test_execute_echo() {
        let executor = create_test_executor().await;
        let result = executor.execute("echo hello").await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn test_parse_command() {
        let executor = create_test_executor().await;
        let (cmd, args) = executor.parse_command("ls -la /tmp");
        assert_eq!(cmd, "ls");
        assert_eq!(args, vec!["-la", "/tmp"]);
    }
}
