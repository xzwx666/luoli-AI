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

use config::AppConfig;
use security::{SecurityManager, ExecutionMode};
use monitor::ActivityMonitor;
use executor::CommandExecutor;
use tui::run_tui;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    info!("启动洛璃终端助手...");

    let cli = Cli::parse();
    let config = AppConfig::load()?;
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
        Commands::Init => {
            AppConfig::init_default()?;
            info!("配置文件已初始化");
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
