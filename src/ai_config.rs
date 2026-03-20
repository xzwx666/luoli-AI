use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::{Result, Context};
use log::{info, warn, error};

/// AI Provider 类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    OpenAi,
    DeepSeek,
    Ollama,
    ChatGpt,
    Custom,
}

impl std::fmt::Display for AiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiProvider::OpenAi => write!(f, "OpenAI"),
            AiProvider::DeepSeek => write!(f, "DeepSeek"),
            AiProvider::Ollama => write!(f, "Ollama"),
            AiProvider::ChatGpt => write!(f, "ChatGPT"),
            AiProvider::Custom => write!(f, "Custom"),
        }
    }
}

impl AiProvider {
    /// 获取默认 API 端点
    pub fn default_endpoint(&self) -> String {
        match self {
            AiProvider::OpenAi => "https://api.openai.com/v1".to_string(),
            AiProvider::DeepSeek => "https://api.deepseek.com/v1".to_string(),
            AiProvider::Ollama => "http://localhost:11434".to_string(),
            AiProvider::ChatGpt => "https://api.openai.com/v1".to_string(),
            AiProvider::Custom => "".to_string(),
        }
    }
    
    /// 获取 Provider 的所有支持模型
    pub async fn fetch_models(&self, config: &AiProviderConfig) -> Result<Vec<String>> {
        match self {
            AiProvider::Ollama => Self::fetch_ollama_models(config).await,
            AiProvider::OpenAi | AiProvider::ChatGpt => Self::fetch_openai_models(config).await,
            AiProvider::DeepSeek => Self::fetch_deepseek_models(config).await,
            AiProvider::Custom => Ok(vec![]),
        }
    }
    
    async fn fetch_ollama_models(config: &AiProviderConfig) -> Result<Vec<String>> {
        let client = reqwest::Client::new();
        let url = format!("{}/api/tags", config.api_endpoint);
        
        let response = client
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .context("无法连接到 Ollama 服务")?;
        
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Ollama API 返回错误: {}", response.status()));
        }
        
        let data: serde_json::Value = response.json().await?;
        let models = data["models"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        
        Ok(models)
    }
    
    async fn fetch_openai_models(config: &AiProviderConfig) -> Result<Vec<String>> {
        let client = reqwest::Client::new();
        let url = format!("{}/models", config.api_endpoint);
        
        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", config.api_key))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .context("无法连接到 OpenAI API")?;
        
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("OpenAI API 返回错误: {}", response.status()));
        }
        
        let data: serde_json::Value = response.json().await?;
        let models = data["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        
        Ok(models)
    }
    
    async fn fetch_deepseek_models(config: &AiProviderConfig) -> Result<Vec<String>> {
        // DeepSeek 使用 OpenAI 兼容的 API
        Self::fetch_openai_models(config).await
    }
}

/// AI Provider 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderConfig {
    pub name: String,
    pub provider: AiProvider,
    pub api_endpoint: String,
    pub api_key: String,
    pub default_model: Option<String>,
    pub enabled: bool,
    pub timeout_secs: u64,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub custom_headers: HashMap<String, String>,
}

impl AiProviderConfig {
    pub fn new(name: String, provider: AiProvider) -> Self {
        let api_endpoint = provider.default_endpoint();
        
        Self {
            name,
            provider,
            api_endpoint,
            api_key: String::new(),
            default_model: None,
            enabled: true,
            timeout_secs: 30,
            max_tokens: None,
            temperature: None,
            custom_headers: HashMap::new(),
        }
    }
    
    /// 设置 API 密钥
    pub fn with_api_key(mut self, key: String) -> Self {
        self.api_key = key;
        self
    }
    
    /// 设置 API 端点
    pub fn with_endpoint(mut self, endpoint: String) -> Self {
        self.api_endpoint = endpoint;
        self
    }
    
    /// 设置默认模型
    pub fn with_default_model(mut self, model: String) -> Self {
        self.default_model = Some(model);
        self
    }
    
    /// 验证配置是否有效
    pub fn validate(&self) -> Result<()> {
        if self.api_endpoint.is_empty() {
            return Err(anyhow::anyhow!("API 端点不能为空"));
        }
        
        if self.provider != AiProvider::Ollama && self.api_key.is_empty() {
            warn!("{} 需要 API 密钥", self.provider);
        }
        
        Ok(())
    }
    
    /// 测试连接
    pub async fn test_connection(&self) -> Result<bool> {
        match self.provider.fetch_models(self).await {
            Ok(models) => {
                info!("{} 连接成功，可用模型: {}", self.name, models.len());
                Ok(true)
            }
            Err(e) => {
                error!("{} 连接失败: {}", self.name, e);
                Ok(false)
            }
        }
    }
}

/// AI 配置管理器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfigManager {
    pub providers: HashMap<String, AiProviderConfig>,
    pub active_provider: Option<String>,
    pub global_settings: AiGlobalSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiGlobalSettings {
    pub default_max_tokens: u32,
    pub default_temperature: f32,
    pub request_timeout_secs: u64,
    pub retry_attempts: u32,
    pub enable_streaming: bool,
}

impl Default for AiGlobalSettings {
    fn default() -> Self {
        Self {
            default_max_tokens: 2048,
            default_temperature: 0.7,
            request_timeout_secs: 60,
            retry_attempts: 3,
            enable_streaming: true,
        }
    }
}

impl AiConfigManager {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            active_provider: None,
            global_settings: AiGlobalSettings::default(),
        }
    }
    
    /// 添加 Provider
    pub fn add_provider(&mut self, config: AiProviderConfig) -> Result<()> {
        config.validate()?;
        
        let name = config.name.clone();
        info!("添加 AI Provider: {}", name);
        
        self.providers.insert(name.clone(), config);
        
        // 如果是第一个 Provider，设为默认
        if self.active_provider.is_none() {
            self.active_provider = Some(name);
        }
        
        Ok(())
    }
    
    /// 移除 Provider
    pub fn remove_provider(&mut self, name: &str) -> Result<()> {
        if self.providers.remove(name).is_none() {
            return Err(anyhow::anyhow!("Provider '{}' 不存在", name));
        }
        
        info!("移除 AI Provider: {}", name);
        
        // 如果移除的是当前激活的 Provider，重新选择
        if self.active_provider.as_ref() == Some(&name.to_string()) {
            self.active_provider = self.providers.keys().next().cloned();
        }
        
        Ok(())
    }
    
    /// 获取 Provider
    pub fn get_provider(&self, name: &str) -> Option<&AiProviderConfig> {
        self.providers.get(name)
    }
    
    /// 获取当前激活的 Provider
    pub fn get_active_provider(&self) -> Option<&AiProviderConfig> {
        self.active_provider.as_ref()
            .and_then(|name| self.providers.get(name))
    }
    
    /// 设置激活的 Provider
    pub fn set_active_provider(&mut self, name: &str) -> Result<()> {
        if !self.providers.contains_key(name) {
            return Err(anyhow::anyhow!("Provider '{}' 不存在", name));
        }
        
        self.active_provider = Some(name.to_string());
        info!("切换激活的 AI Provider: {}", name);
        
        Ok(())
    }
    
    /// 列出所有 Providers
    pub fn list_providers(&self) -> Vec<(&String, &AiProviderConfig)> {
        self.providers.iter().collect()
    }
    
    /// 更新 Provider 配置
    pub fn update_provider(&mut self, name: &str, config: AiProviderConfig) -> Result<()> {
        if !self.providers.contains_key(name) {
            return Err(anyhow::anyhow!("Provider '{}' 不存在", name));
        }
        
        config.validate()?;
        self.providers.insert(name.to_string(), config);
        info!("更新 AI Provider: {}", name);
        
        Ok(())
    }
    
    /// 加载指定 Provider 的可用模型
    pub async fn load_models(&self, provider_name: &str) -> Result<Vec<String>> {
        let provider = self.providers.get(provider_name)
            .ok_or_else(|| anyhow::anyhow!("Provider '{}' 不存在", provider_name))?;
        
        provider.provider.fetch_models(provider).await
    }
    
    /// 获取所有 Provider 的模型列表
    pub async fn load_all_models(&self) -> HashMap<String, Vec<String>> {
        let mut all_models = HashMap::new();
        
        for (name, config) in &self.providers {
            match config.provider.fetch_models(config).await {
                Ok(models) => {
                    all_models.insert(name.clone(), models);
                }
                Err(e) => {
                    warn!("无法加载 {} 的模型: {}", name, e);
                    all_models.insert(name.clone(), vec![]);
                }
            }
        }
        
        all_models
    }
    
    /// 创建默认配置
    pub fn create_default_configs() -> Vec<AiProviderConfig> {
        vec![
            AiProviderConfig::new("Ollama 本地".to_string(), AiProvider::Ollama)
                .with_endpoint("http://localhost:11434".to_string()),
            AiProviderConfig::new("OpenAI".to_string(), AiProvider::OpenAi),
            AiProviderConfig::new("DeepSeek".to_string(), AiProvider::DeepSeek),
            AiProviderConfig::new("ChatGPT".to_string(), AiProvider::ChatGpt),
        ]
    }
    
    /// 设置指定 Provider 的默认模型
    pub fn set_default_model(&mut self, name: &str, model: &str) -> Result<()> {
        let provider = self.providers.get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("Provider '{}' 不存在", name))?;
        
        provider.default_model = Some(model.to_string());
        info!("为 {} 设置默认模型: {}", name, model);
        Ok(())
    }
    
    /// 测试指定 Provider 的连接
    pub async fn test_connection(&self, name: &str) -> Result<bool> {
        let provider = self.providers.get(name)
            .ok_or_else(|| anyhow::anyhow!("Provider '{}' 不存在", name))?;
        
        provider.test_connection().await
    }
    
    /// 保存配置到文件
    pub fn save(&self) -> Result<()> {
        let config_path = Self::ai_config_path()?;
        
        // 确保配置目录存在
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        info!("AI 配置已保存到: {:?}", config_path);
        
        Ok(())
    }
    
    /// 从文件加载配置
    pub fn load() -> Result<Self> {
        let config_path = Self::ai_config_path()?;
        
        if !config_path.exists() {
            return Ok(Self::new());
        }
        
        let content = std::fs::read_to_string(&config_path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }
    
    fn ai_config_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("无法获取用户主目录"))?;
        Ok(home.join(".luoli").join("ai_config.json"))
    }
}

impl Default for AiConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ai_provider_display() {
        assert_eq!(AiProvider::OpenAi.to_string(), "OpenAI");
        assert_eq!(AiProvider::DeepSeek.to_string(), "DeepSeek");
        assert_eq!(AiProvider::Ollama.to_string(), "Ollama");
    }
    
    #[test]
    fn test_provider_config_validation() {
        let config = AiProviderConfig::new("Test".to_string(), AiProvider::OpenAi);
        assert!(config.validate().is_ok());
        
        let mut invalid_config = config.clone();
        invalid_config.api_endpoint = "".to_string();
        assert!(invalid_config.validate().is_err());
    }
    
    #[test]
    fn test_config_manager() {
        let mut manager = AiConfigManager::new();
        
        let config = AiProviderConfig::new("Test".to_string(), AiProvider::Ollama);
        manager.add_provider(config).unwrap();
        
        assert_eq!(manager.providers.len(), 1);
        assert!(manager.get_provider("Test").is_some());
        
        manager.remove_provider("Test").unwrap();
        assert_eq!(manager.providers.len(), 0);
    }
}
