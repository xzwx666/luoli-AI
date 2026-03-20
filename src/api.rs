use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::{Result, Context};
use log::{info, warn, error};

use crate::config::{ApiConfig, ApiEndpoint};
use crate::security::SecurityManager;
use crate::monitor::{ActivityMonitor, ActivityRecord, ActivityType, ActivityStatus};

/// API 客户端
pub struct ApiClient {
    client: reqwest::Client,
    security: Arc<RwLock<SecurityManager>>,
    monitor: Arc<ActivityMonitor>,
    endpoints: HashMap<String, ApiEndpoint>,
}

impl ApiClient {
    pub fn new(
        config: ApiConfig,
        security: Arc<RwLock<SecurityManager>>,
        monitor: Arc<ActivityMonitor>,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("创建 HTTP 客户端失败")?;

        let mut endpoints = HashMap::new();
        for endpoint in config.endpoints {
            endpoints.insert(endpoint.name.clone(), endpoint);
        }

        info!("API 客户端初始化完成，已配置 {} 个端点", endpoints.len());

        Ok(Self {
            client,
            security,
            monitor,
            endpoints,
        })
    }

    /// 发送 HTTP 请求
    pub async fn request(
        &self,
        url: &str,
        method: &str,
        headers: Option<HashMap<String, String>>,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let start_time = std::time::Instant::now();

        // 安全检查
        {
            let security = self.security.read().await;
            security.check_api(url)?;
        }

        // 创建活动记录
        let mut record = ActivityRecord::new(
            ActivityType::ApiCall,
            whoami::username(),
            format!("{} {}", method.to_uppercase(), url),
            vec![],
        );

        // 构建请求
        let mut request_builder = match method.to_uppercase().as_str() {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "DELETE" => self.client.delete(url),
            "PATCH" => self.client.patch(url),
            _ => return Err(anyhow::anyhow!("不支持的 HTTP 方法: {}", method)),
        };

        // 添加请求头
        if let Some(hdrs) = headers {
            for (key, value) in hdrs {
                request_builder = request_builder.header(&key, value);
            }
        }

        // 添加请求体
        if let Some(b) = body {
            request_builder = request_builder.json(&b);
        }

        // 发送请求
        info!("发送 {} 请求到: {}", method.to_uppercase(), url);
        let response = request_builder.send().await;

        // 处理响应
        let duration = start_time.elapsed().as_millis() as u64;
        record = record.with_duration(duration);

        match response {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();

                let result = if status.is_success() {
                    record = record.with_status(ActivityStatus::Success);
                    
                    // 尝试解析为 JSON
                    match serde_json::from_str::<serde_json::Value>(&body) {
                        Ok(json) => json,
                        Err(_) => serde_json::json!({ "text": body }),
                    }
                } else {
                    let error_msg = format!("HTTP {}: {}", status, body);
                    record = record.with_status(ActivityStatus::Failed(error_msg.clone()));
                    return Err(anyhow::anyhow!(error_msg));
                };

                // 记录活动
                self.monitor.log_activity(record).await?;

                info!("API 请求成功: {} ({}ms)", url, duration);
                Ok(result)
            }
            Err(e) => {
                let error_msg = format!("请求失败: {}", e);
                record = record.with_status(ActivityStatus::Failed(error_msg.clone()));
                
                // 记录活动
                self.monitor.log_activity(record).await?;
                
                error!("API 请求失败: {}", e);
                Err(anyhow::anyhow!(error_msg))
            }
        }
    }

    /// 使用预配置的端点发送请求
    pub async fn request_endpoint(
        &self,
        endpoint_name: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let endpoint = self.endpoints.get(endpoint_name)
            .ok_or_else(|| anyhow::anyhow!("未知的 API 端点: {}", endpoint_name))?;

        self.request(
            &endpoint.url,
            &endpoint.method,
            Some(endpoint.headers.clone()),
            body,
        ).await
    }

    /// GET 请求
    pub async fn get(&self, url: &str) -> Result<serde_json::Value> {
        self.request(url, "GET", None, None).await
    }

    /// POST 请求
    pub async fn post(&self, url: &str, body: serde_json::Value) -> Result<serde_json::Value> {
        self.request(url, "POST", None, Some(body)).await
    }

    /// 添加自定义端点
    pub fn add_endpoint(&mut self, endpoint: ApiEndpoint) {
        info!("添加 API 端点: {}", endpoint.name);
        self.endpoints.insert(endpoint.name.clone(), endpoint);
    }

    /// 列出所有端点
    pub fn list_endpoints(&self) -> Vec<&ApiEndpoint> {
        self.endpoints.values().collect()
    }
}

/// GitHub API 封装
pub struct GitHubApi {
    client: ApiClient,
    base_url: String,
}

impl GitHubApi {
    pub fn new(client: ApiClient) -> Self {
        Self {
            client,
            base_url: "https://api.github.com".to_string(),
        }
    }

    /// 获取用户信息
    pub async fn get_user(&self, username: &str) -> Result<serde_json::Value> {
        let url = format!("{}/users/{}", self.base_url, username);
        self.client.get(&url).await
    }

    /// 获取仓库信息
    pub async fn get_repo(&self, owner: &str, repo: &str) -> Result<serde_json::Value> {
        let url = format!("{}/repos/{}/{}", self.base_url, owner, repo);
        self.client.get(&url).await
    }

    /// 列出仓库文件
    pub async fn list_contents(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/repos/{}/{}/contents/{}", self.base_url, owner, repo, path);
        self.client.get(&url).await
    }
}

/// OpenAI API 封装
pub struct OpenAiApi {
    client: ApiClient,
    api_key: String,
    base_url: String,
}

impl OpenAiApi {
    pub fn new(client: ApiClient, api_key: String) -> Self {
        Self {
            client,
            api_key,
            base_url: "https://api.openai.com/v1".to_string(),
        }
    }

    /// 发送聊天请求
    pub async fn chat_completion(
        &self,
        model: &str,
        messages: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/chat/completions", self.base_url);
        
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), format!("Bearer {}", self.api_key));
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let body = serde_json::json!({
            "model": model,
            "messages": messages,
        });

        self.client.request(&url, "POST", Some(headers), Some(body)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{SecurityConfig, MonitorConfig};
    use std::path::PathBuf;

    async fn create_test_client() -> ApiClient {
        let security_config = SecurityConfig {
            default_mode: "normal".to_string(),
            command_allowlist: vec![],
            command_denylist: vec![],
            tool_allowlist: vec![],
            tool_denylist: vec![],
            api_allowlist: vec!["https://api.github.com".to_string()],
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

        let api_config = ApiConfig {
            enabled: true,
            endpoints: vec![],
        };

        let security = Arc::new(RwLock::new(
            crate::security::SecurityManager::new(security_config).unwrap()
        ));
        let monitor = Arc::new(
            crate::monitor::ActivityMonitor::new(monitor_config).unwrap()
        );

        ApiClient::new(api_config, security, monitor).unwrap()
    }
}
