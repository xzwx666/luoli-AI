use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use anyhow::{Result, Context};
use log::{info, warn, error};

use crate::config::MonitorConfig;

/// 活动记录类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivityType {
    CommandExecution,
    ToolInvocation,
    ApiCall,
    FileAccess,
    ModeChange,
    ConfigChange,
    SecurityViolation,
}

impl std::fmt::Display for ActivityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActivityType::CommandExecution => write!(f, "命令执行"),
            ActivityType::ToolInvocation => write!(f, "工具调用"),
            ActivityType::ApiCall => write!(f, "API调用"),
            ActivityType::FileAccess => write!(f, "文件访问"),
            ActivityType::ModeChange => write!(f, "模式切换"),
            ActivityType::ConfigChange => write!(f, "配置变更"),
            ActivityType::SecurityViolation => write!(f, "安全违规"),
        }
    }
}

/// 活动记录状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivityStatus {
    Success,
    Failed(String),
    Blocked(String),
    Pending,
}

impl std::fmt::Display for ActivityStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActivityStatus::Success => write!(f, "成功"),
            ActivityStatus::Failed(reason) => write!(f, "失败: {}", reason),
            ActivityStatus::Blocked(reason) => write!(f, "阻止: {}", reason),
            ActivityStatus::Pending => write!(f, "进行中"),
        }
    }
}

/// 活动记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityRecord {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub activity_type: ActivityType,
    pub user: String,
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    pub status: ActivityStatus,
    pub duration_ms: Option<u64>,
    pub output: Option<String>,
    pub env_vars: Option<Vec<(String, String)>>,
    pub metadata: serde_json::Value,
}

impl ActivityRecord {
    pub fn new(
        activity_type: ActivityType,
        user: String,
        command: String,
        args: Vec<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            activity_type,
            user,
            command,
            args,
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            status: ActivityStatus::Pending,
            duration_ms: None,
            output: None,
            env_vars: None,
            metadata: serde_json::json!({}),
        }
    }

    pub fn with_output(mut self, output: String) -> Self {
        self.output = Some(output);
        self
    }

    pub fn with_status(mut self, status: ActivityStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    pub fn with_env_vars(mut self, env_vars: Vec<(String, String)>) -> Self {
        self.env_vars = Some(env_vars);
        self
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// 活动监控器
pub struct ActivityMonitor {
    config: MonitorConfig,
    records: Arc<RwLock<VecDeque<ActivityRecord>>>,
    log_file: Arc<RwLock<fs::File>>,
}

impl ActivityMonitor {
    pub fn new(config: MonitorConfig) -> Result<Self> {
        // 确保日志目录存在
        if let Some(parent) = config.log_file.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("创建日志目录失败: {:?}", parent))?;
        }

        // 打开或创建日志文件
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.log_file)
            .with_context(|| format!("打开日志文件失败: {:?}", config.log_file))?;

        info!("活动监控器初始化完成，日志文件: {:?}", config.log_file);

        Ok(Self {
            config,
            records: Arc::new(RwLock::new(VecDeque::new())),
            log_file: Arc::new(RwLock::new(log_file)),
        })
    }

    /// 记录活动
    pub async fn log_activity(&self, record: ActivityRecord) -> Result<()> {
        // 写入内存缓存
        {
            let mut records = self.records.write().await;
            records.push_back(record.clone());
            
            // 限制内存中保留的记录数
            while records.len() > 1000 {
                records.pop_front();
            }
        }

        // 写入日志文件
        if self.config.enabled {
            self.write_to_log(&record).await?;
        }

        // 检查是否需要轮转日志
        self.check_rotation().await?;

        Ok(())
    }

    /// 写入日志文件
    async fn write_to_log(&self, record: &ActivityRecord) -> Result<()> {
        let log_entry = serde_json::to_string(record)?;
        
        let mut file = self.log_file.write().await;
        writeln!(file, "{}", log_entry)?;
        file.flush()?;
        
        Ok(())
    }

    /// 检查并执行日志轮转
    async fn check_rotation(&self) -> Result<()> {
        let metadata = fs::metadata(&self.config.log_file)?;
        
        if metadata.len() > self.config.max_log_size as u64 {
            self.rotate_logs().await?;
        }
        
        Ok(())
    }

    /// 执行日志轮转
    async fn rotate_logs(&self) -> Result<()> {
        let log_path = &self.config.log_file;
        
        // 删除最旧的日志文件
        let oldest = format!("{}.{}.{}", log_path.display(), self.config.log_rotation, "gz");
        if std::path::Path::new(&oldest).exists() {
            fs::remove_file(&oldest)?;
        }

        // 轮转现有日志文件
        for i in (1..self.config.log_rotation).rev() {
            let old_path = format!("{}.{}", log_path.display(), i);
            let new_path = format!("{}.{}", log_path.display(), i + 1);
            
            if std::path::Path::new(&old_path).exists() {
                fs::rename(&old_path, &new_path)?;
            }
        }

        // 重命名当前日志文件
        let backup_path = format!("{}.{}", log_path.display(), 1);
        fs::rename(log_path, &backup_path)?;

        // 创建新的日志文件
        let new_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;

        *self.log_file.write().await = new_file;

        info!("日志轮转完成");
        Ok(())
    }

    /// 获取最近的记录
    pub async fn get_recent_records(&self, limit: usize) -> Vec<ActivityRecord> {
        let records = self.records.read().await;
        records.iter().rev().take(limit).cloned().collect()
    }

    /// 显示日志
    pub async fn show_logs(&self, limit: usize) -> Result<()> {
        let records = self.get_recent_records(limit).await;
        
        println!("\n=== 最近 {} 条活动记录 ===\n", records.len());
        
        for record in records {
            let local_time: DateTime<Local> = record.timestamp.into();
            println!("[{}] {} - {}", 
                local_time.format("%Y-%m-%d %H:%M:%S"),
                record.activity_type,
                record.id[..8].to_string()
            );
            println!("  用户: {}", record.user);
            println!("  命令: {} {}", 
                record.command,
                record.args.join(" ")
            );
            println!("  工作目录: {:?}", record.working_dir);
            println!("  状态: {}", record.status);
            
            if let Some(duration) = record.duration_ms {
                println!("  耗时: {}ms", duration);
            }
            
            if let Some(output) = &record.output {
                let preview: String = output.chars().take(200).collect();
                println!("  输出预览: {}", preview);
                if output.len() > 200 {
                    println!("  ... ({} 字符)", output.len());
                }
            }
            
            println!();
        }
        
        Ok(())
    }

    /// 搜索记录
    pub async fn search_records(
        &self,
        activity_type: Option<ActivityType>,
        status: Option<ActivityStatus>,
        keyword: Option<String>,
    ) -> Vec<ActivityRecord> {
        let records = self.records.read().await;
        
        records
            .iter()
            .filter(|r| {
                if let Some(ref t) = activity_type {
                    if std::mem::discriminant(&r.activity_type) != std::mem::discriminant(t) {
                        return false;
                    }
                }
                if let Some(ref s) = status {
                    if std::mem::discriminant(&r.status) != std::mem::discriminant(s) {
                        return false;
                    }
                }
                if let Some(ref k) = keyword {
                    if !r.command.contains(k) && !r.args.iter().any(|a| a.contains(k)) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect()
    }

    /// 导出日志到文件
    pub async fn export_logs(&self, output_path: &PathBuf, format: &str) -> Result<()> {
        let records = self.records.read().await;
        
        match format {
            "json" => {
                let json = serde_json::to_string_pretty(&records.iter().collect::<Vec<_>>())?;
                fs::write(output_path, json)?;
            }
            "csv" => {
                let mut wtr = csv::Writer::from_path(output_path)?;
                for record in records.iter() {
                    wtr.serialize(record)?;
                }
                wtr.flush()?;
            }
            _ => return Err(anyhow::anyhow!("不支持的导出格式: {}", format)),
        }
        
        info!("日志已导出到: {:?}", output_path);
        Ok(())
    }

    /// 获取统计信息
    pub async fn get_statistics(&self) -> serde_json::Value {
        let records = self.records.read().await;
        
        let total = records.len();
        let success = records.iter().filter(|r| matches!(r.status, ActivityStatus::Success)).count();
        let failed = records.iter().filter(|r| matches!(r.status, ActivityStatus::Failed(_))).count();
        let blocked = records.iter().filter(|r| matches!(r.status, ActivityStatus::Blocked(_))).count();
        
        let command_count = records.iter().filter(|r| matches!(r.activity_type, ActivityType::CommandExecution)).count();
        let tool_count = records.iter().filter(|r| matches!(r.activity_type, ActivityType::ToolInvocation)).count();
        let api_count = records.iter().filter(|r| matches!(r.activity_type, ActivityType::ApiCall)).count();
        let violation_count = records.iter().filter(|r| matches!(r.activity_type, ActivityType::SecurityViolation)).count();
        
        serde_json::json!({
            "total_records": total,
            "success": success,
            "failed": failed,
            "blocked": blocked,
            "by_type": {
                "command": command_count,
                "tool": tool_count,
                "api": api_count,
                "violation": violation_count,
            }
        })
    }
}

/// 监控中间件 trait
#[async_trait::async_trait]
pub trait Monitored {
    async fn execute_monitored<F, T>(
        &self,
        activity_type: ActivityType,
        command: String,
        args: Vec<String>,
        operation: F,
    ) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>> + Send,
        T: Send;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_config() -> (MonitorConfig, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let log_file = temp_dir.path().join("test.log");
        
        let config = MonitorConfig {
            enabled: true,
            log_file,
            max_log_size: 1024 * 1024, // 1MB
            log_rotation: 3,
            capture_output: true,
            capture_env: false,
        };
        
        (config, temp_dir)
    }

    #[tokio::test]
    async fn test_log_activity() {
        let (config, _temp) = create_test_config();
        let monitor = ActivityMonitor::new(config).unwrap();
        
        let record = ActivityRecord::new(
            ActivityType::CommandExecution,
            "test_user".to_string(),
            "ls".to_string(),
            vec!["-la".to_string()],
        ).with_status(ActivityStatus::Success);
        
        monitor.log_activity(record).await.unwrap();
        
        let records = monitor.get_recent_records(10).await;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].command, "ls");
    }
}
