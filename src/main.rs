use clap::{Parser, Subcommand};
use log::{info, error};
use std::sync::Arc;
use tokio::sync::RwLock;

mod config;
mod security;
mod monitor;
mod executor;
mod mcp;
mod api;
mod tui;
mod ai_config;

use config::AppConfig;
use security::{SecurityManager, ExecutionMode};
use monitor::ActivityMonitor;
use executor::CommandExecutor;
use tui::run_tui;
use ai_config::{AiProvider, AiProviderConfig};

#[derive(Parser)]
#[command(name = "luoli")]
#[command(about = "洛璃 - 个人终端助手")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 启动交互式终端（简单模式）
    Shell {
        /// 运行模式: strict(严格模式) 或 normal(普通模式)
        #[arg(short, long, default_value = "normal")]
        mode: String,
    },
    /// 启动 TUI 界面（图形终端）
    Tui {
        /// 运行模式: strict(严格模式) 或 normal(普通模式)
        #[arg(short, long, default_value = "normal")]
        mode: String,
    },
    /// 启动 Web 界面
    Web {
        /// 监听端口
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    /// 执行单个命令
    Exec {
        /// 要执行的命令
        command: String,
        /// 运行模式
        #[arg(short, long, default_value = "normal")]
        mode: String,
    },
    /// 管理黑白名单
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// 查看监控日志
    Logs {
        /// 显示最近的N条记录
        #[arg(short, long, default_value = "50")]
        limit: usize,
    },
    /// 初始化配置
    Init,
    /// AI 配置管理
    Ai {
        #[command(subcommand)]
        action: AiAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// 添加命令白名单
    AllowCommand { pattern: String },
    /// 添加命令黑名单
    DenyCommand { pattern: String },
    /// 添加工具白名单
    AllowTool { name: String },
    /// 添加工具黑名单
    DenyTool { name: String },
    /// 列出所有规则
    List,
    /// 切换运行模式
    SetMode { mode: String },
}

#[derive(Subcommand, Clone)]
enum AiAction {
    /// 添加 AI Provider
    Add {
        /// Provider 名称（自定义）
        name: String,
        /// Provider 类型: openai, deepseek, ollama, chatgpt, custom
        #[arg(short, long)]
        provider: String,
        /// API 密钥
        #[arg(short, long)]
        api_key: String,
        /// API 端点地址
        #[arg(short, long)]
        endpoint: Option<String>,
        /// 默认模型（可选）
        #[arg(short, long)]
        model: Option<String>,
    },
    /// 删除 AI Provider
    Remove {
        /// Provider 名称
        name: String,
    },
    /// 列出所有配置的 Providers
    List,
    /// 测试连接
    Test {
        /// Provider 名称
        name: String,
    },
    /// 加载并显示可用模型
    Models {
        /// Provider 名称
        name: String,
    },
    /// 设置当前使用的 Provider
    Use {
        /// Provider 名称
        name: String,
    },
    /// 设置默认模型
    SetModel {
        /// Provider 名称
        name: String,
        /// 模型名称
        model: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    info!("启动洛璃终端助手...");

    let cli = Cli::parse();
    let config = AppConfig::load()?;
    
    // 处理不需要完整初始化的命令
    match &cli.command {
        Commands::Ai { action } => {
            return handle_ai_config(action.clone(), &config).await;
        }
        Commands::Init => {
            AppConfig::init_default()?;
            info!("配置文件已初始化");
            return Ok(());
        }
        _ => {}
    }
    
    let security = Arc::new(RwLock::new(SecurityManager::new(config.security)?));
    let monitor = Arc::new(ActivityMonitor::new(config.monitor)?);
    let executor = Arc::new(CommandExecutor::new(security.clone(), monitor.clone()));

    match cli.command {
        Commands::Shell { mode } => {
            let mode = parse_mode(&mode)?;
            security.write().await.set_mode(mode);
            info!("启动交互式终端，模式: {:?}", mode);
            run_shell(executor, security, monitor).await?;
        }
        Commands::Tui { mode } => {
            let mode = parse_mode(&mode)?;
            security.write().await.set_mode(mode);
            info!("启动 TUI 界面，模式: {:?}", mode);
            run_tui().await?;
        }
        Commands::Web { port } => {
            info!("启动 Web 界面，端口: {}", port);
            println!("Web 界面已启动: http://localhost:{}", port);
            println!("请使用浏览器访问上述地址");
            // Web 服务器将在后续实现
        }
        Commands::Exec { command, mode } => {
            let mode = parse_mode(&mode)?;
            security.write().await.set_mode(mode);
            executor.execute(&command).await?;
        }
        Commands::Config { action } => {
            handle_config(action, security).await?;
        }
        Commands::Logs { limit } => {
            monitor.show_logs(limit).await?;
        }
        Commands::Init | Commands::Ai { .. } => {
            // 已在前面处理
        }
    }

    Ok(())
}

fn parse_mode(mode: &str) -> anyhow::Result<ExecutionMode> {
    match mode.to_lowercase().as_str() {
        "strict" | "严格" => Ok(ExecutionMode::Strict),
        "normal" | "普通" => Ok(ExecutionMode::Normal),
        _ => Err(anyhow::anyhow!("未知模式: {}. 请使用 'strict' 或 'normal'", mode)),
    }
}

async fn run_shell(
    executor: Arc<CommandExecutor>,
    security: Arc<RwLock<SecurityManager>>,
    monitor: Arc<ActivityMonitor>,
) -> anyhow::Result<()> {
    use std::io::{self, Write};
    
    println!("🦞 洛璃终端助手已启动");
    println!("当前模式: {:?}", security.read().await.mode());
    println!("输入 'help' 查看帮助, 'exit' 退出\n");

    loop {
        print!("luoli> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        match input {
            "exit" | "quit" => {
                println!("再见!");
                break;
            }
            "help" => {
                print_help();
            }
            "mode" => {
                println!("当前模式: {:?}", security.read().await.mode());
            }
            cmd => {
                if let Err(e) = executor.execute(cmd).await {
                    error!("执行失败: {}", e);
                    println!("错误: {}", e);
                }
            }
        }
    }

    Ok(())
}

fn print_help() {
    println!("\n可用命令:");
    println!("  help       - 显示帮助");
    println!("  mode       - 查看当前模式");
    println!("  exit/quit  - 退出终端");
    println!("\n其他任意命令将被执行（根据当前模式进行权限检查）\n");
}

async fn handle_config(
    action: ConfigAction,
    security: Arc<RwLock<SecurityManager>>,
) -> anyhow::Result<()> {
    let mut sec = security.write().await;
    
    match action {
        ConfigAction::AllowCommand { pattern } => {
            sec.add_command_allowlist(&pattern)?;
            println!("已添加命令白名单: {}", pattern);
        }
        ConfigAction::DenyCommand { pattern } => {
            sec.add_command_denylist(&pattern)?;
            println!("已添加命令黑名单: {}", pattern);
        }
        ConfigAction::AllowTool { name } => {
            sec.add_tool_allowlist(&name)?;
            println!("已添加工具白名单: {}", name);
        }
        ConfigAction::DenyTool { name } => {
            sec.add_tool_denylist(&name)?;
            println!("已添加工具黑名单: {}", name);
        }
        ConfigAction::List => {
            sec.list_rules();
        }
        ConfigAction::SetMode { mode } => {
            let mode = parse_mode(&mode)?;
            sec.set_mode(mode);
            println!("已切换到 {:?} 模式", mode);
        }
    }
    
    sec.save_config()?;
    Ok(())
}

async fn handle_ai_config(
    action: AiAction,
    config: &AppConfig,
) -> anyhow::Result<()> {
    let mut ai_manager = config.ai.clone();
    
    match action {
        AiAction::Add { name, provider, api_key, endpoint, model } => {
            let provider_type = parse_provider_type(&provider)?;
            
            // 根据 provider 类型设置默认端点
            let api_endpoint = endpoint.unwrap_or_else(|| {
                match provider_type {
                    AiProvider::OpenAi => "https://api.openai.com/v1".to_string(),
                    AiProvider::DeepSeek => "https://api.deepseek.com/v1".to_string(),
                    AiProvider::ChatGpt => "https://api.openai.com/v1".to_string(),
                    AiProvider::Ollama => "http://localhost:11434".to_string(),
                    AiProvider::Custom => "http://localhost:8000/v1".to_string(),
                }
            });
            
            let provider_config = AiProviderConfig {
                name: name.clone(),
                provider: provider_type,
                api_endpoint,
                api_key,
                default_model: model,
                enabled: true,
                timeout_secs: 30,
                max_tokens: Some(4096),
                temperature: Some(0.7),
                custom_headers: std::collections::HashMap::new(),
            };
            
            ai_manager.add_provider(provider_config)?;
            ai_manager.save()?;
            println!("✅ 已添加 AI Provider: {}", name);
            println!("   类型: {}", provider);
            println!("   可以使用 `luoli ai test {}` 测试连接", name);
        }
        AiAction::Remove { name } => {
            ai_manager.remove_provider(&name)?;
            ai_manager.save()?;
            println!("✅ 已删除 AI Provider: {}", name);
        }
        AiAction::List => {
            let providers = ai_manager.list_providers();
            if providers.is_empty() {
                println!("⚠️  未配置任何 AI Provider");
                println!("   使用 `luoli ai add <名称> --provider <类型> --api-key <密钥>` 添加");
            } else {
                println!("\n📋 已配置的 AI Providers:\n");
                for (name, config) in providers {
                    let active_marker = if ai_manager.get_active_provider().map(|p| p.name == *name).unwrap_or(false) {
                        "🟢 "
                    } else if config.enabled {
                        "⚪ "
                    } else {
                        "⚫ "
                    };
                    println!("{} {}", active_marker, name);
                    println!("   类型: {:?}", config.provider);
                    println!("   端点: {}", config.api_endpoint);
                    if let Some(ref model) = config.default_model {
                        println!("   默认模型: {}", model);
                    }
                    println!();
                }
            }
        }
        AiAction::Test { name } => {
            match ai_manager.get_provider(&name) {
                Some(config) => {
                    println!("🔄 正在测试连接: {} ...", name);
                    match ai_manager.test_connection(&name).await {
                        Ok(true) => {
                            println!("✅ 连接成功!");
                            println!("   Provider: {:?}", config.provider);
                            println!("   端点: {}", config.api_endpoint);
                        }
                        Ok(false) => {
                            println!("❌ 连接失败: 无法连接到 API 端点");
                        }
                        Err(e) => {
                            println!("❌ 连接错误: {}", e);
                        }
                    }
                }
                None => {
                    println!("❌ 未找到 Provider: {}", name);
                    println!("   使用 `luoli ai list` 查看所有配置的 Providers");
                }
            }
        }
        AiAction::Models { name } => {
            match ai_manager.get_provider(&name) {
                Some(_) => {
                    println!("🔄 正在加载模型列表: {} ...", name);
                    match ai_manager.load_models(&name).await {
                        Ok(models) => {
                            if models.is_empty() {
                                println!("⚠️  未找到可用模型");
                            } else {
                                println!("\n📚 可用模型 ({} 个):\n", models.len());
                                for (i, model) in models.iter().enumerate() {
                                    println!("   {}. {}", i + 1, model);
                                }
                                println!("\n   使用 `luoli ai set-model {} <模型名>` 设置默认模型", name);
                            }
                        }
                        Err(e) => {
                            println!("❌ 加载模型失败: {}", e);
                        }
                    }
                }
                None => {
                    println!("❌ 未找到 Provider: {}", name);
                }
            }
        }
        AiAction::Use { name } => {
            match ai_manager.set_active_provider(&name) {
                Ok(_) => {
                    ai_manager.save()?;
                    println!("✅ 已切换到 Provider: {}", name);
                }
                Err(e) => {
                    println!("❌ 切换失败: {}", e);
                }
            }
        }
        AiAction::SetModel { name, model } => {
            match ai_manager.get_provider(&name) {
                Some(_) => {
                    ai_manager.set_default_model(&name, &model)?;
                    ai_manager.save()?;
                    println!("✅ 已为 {} 设置默认模型: {}", name, model);
                }
                None => {
                    println!("❌ 未找到 Provider: {}", name);
                }
            }
        }
    }
    
    Ok(())
}

fn parse_provider_type(provider: &str) -> anyhow::Result<AiProvider> {
    match provider.to_lowercase().as_str() {
        "openai" => Ok(AiProvider::OpenAi),
        "deepseek" => Ok(AiProvider::DeepSeek),
        "ollama" => Ok(AiProvider::Ollama),
        "chatgpt" => Ok(AiProvider::ChatGpt),
        "custom" => Ok(AiProvider::Custom),
        _ => Err(anyhow::anyhow!("未知的 Provider 类型: {}. 支持的类型: openai, deepseek, ollama, chatgpt, custom", provider)),
    }
}
