//! Rusher 主程序入口

use clap::{Parser, Subcommand};
use rusher::{quick_start, AppInfo, ConfigLoader, Result};

/// Rusher 命令行参数
#[derive(Parser, Debug)]
#[command(
    name = "rusher",
    version = "0.1.0",
    about = "GitHub DNS加速服务 - Rust实现",
    author = "Rusher Team",
    long_about = "GitHub DNS 加速服务 - Rust 实现\n\n使用 GitHub 公开的 IP 范围，扫描所有可用的 IP，\n间隔指定时间检测与记录扫描到的 IP 的访问耗时，\n拦截 DNS，访问 GitHub 时返回最快的 IP。"
)]
struct Cli {
    /// 子命令
    #[command(subcommand)]
    command: Option<Commands>,
    
    /// 配置文件路径
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,
    
    /// 日志级别
    #[arg(short, long, value_name = "LEVEL", default_value = "info")]
    log_level: String,
    
    /// 日志文件路径
    #[arg(long, value_name = "FILE")]
    log_file: Option<String>,
    
    /// 以守护进程模式运行
    #[arg(short, long)]
    daemon: bool,
    
    /// 显示详细输出
    #[arg(short, long)]
    verbose: bool,
}

/// 子命令
#[derive(Subcommand, Debug)]
enum Commands {
    /// 启动服务
    Start {
        /// DNS 监听地址
        #[arg(short, long, value_name = "ADDR:PORT")]
        listen: Option<String>,
    },
    
    /// 停止服务
    Stop,
    
    /// 重启服务
    Restart,
    
    /// 查看服务状态
    Status,
    
    /// 测试配置
    Test {
        /// 配置文件路径
        #[arg(short, long, value_name = "FILE")]
        config: Option<String>,
    },
    
    /// 生成配置文件
    Generate {
        /// 输出文件路径
        #[arg(short, long, value_name = "FILE", default_value = "config/default.toml")]
        output: String,
        
        /// 生成开发环境配置
        #[arg(short, long)]
        development: bool,
    },
    
    /// 扫描 GitHub IP
    Scan {
        /// 扫描次数
        #[arg(short, long, default_value = "1")]
        count: u32,
        
        /// 输出格式
        #[arg(short, long, value_name = "FORMAT", default_value = "text")]
        format: String,
    },
    
    /// 查看版本信息
    Version,
}

/// 主函数
#[tokio::main]
async fn main() -> Result<()> {
    // 解析命令行参数
    let cli = Cli::parse();
    
    // 处理子命令
    if let Some(command) = cli.command {
        match command {
            Commands::Start { listen } => {
                println!("启动 Rusher 服务...");
                if let Some(addr) = listen {
                    println!("监听地址: {}", addr);
                }
                // 启动服务
                rusher::quick_start_with_config(cli.config).await?;
            }
            Commands::Stop => {
                println!("停止 Rusher 服务...");
                // TODO: 实现停止服务逻辑
                println!("服务停止功能尚未实现");
            }
            Commands::Restart => {
                println!("重启 Rusher 服务...");
                // TODO: 实现重启服务逻辑
                println!("服务重启功能尚未实现");
            }
            Commands::Status => {
                println!("查看 Rusher 服务状态...");
                // TODO: 实现状态查看逻辑
                println!("服务状态功能尚未实现");
            }
            Commands::Test { config } => {
                println!("测试 Rusher 配置...");
                test_config(config).await?;
            }
            Commands::Generate { output, development } => {
                println!("生成 Rusher 配置文件...");
                generate_config(&output, development).await?;
            }
            Commands::Scan { count, format } => {
                println!("扫描 GitHub IP...");
                println!("扫描次数: {}", count);
                println!("输出格式: {}", format);
                // TODO: 实现扫描逻辑
                println!("扫描功能尚未实现");
            }
            Commands::Version => {
                let app_info = AppInfo::get();
                app_info.print();
            }
        }
        return Ok(());
    }
    
    // 如果没有子命令，默认启动服务
    println!("启动 Rusher 服务...");
    quick_start().await?;
    
    Ok(())
}

/// 测试配置
async fn test_config(config_path: Option<String>) -> Result<()> {
    println!("=== 配置测试 ===");
    
    let config = if let Some(path) = config_path {
        println!("从文件加载配置: {}", path);
        ConfigLoader::load_from_path(&path)?
    } else {
        println!("从默认位置加载配置");
        ConfigLoader::new().load()?
    };
    
    // 验证配置
    rusher::config::loader::utils::validate_config(&config)?;
    
    // 打印配置摘要
    rusher::config::loader::utils::print_config_summary(&config);
    
    println!("配置测试通过!");
    println!("================");
    
    Ok(())
}

/// 生成配置文件
async fn generate_config(output_path: &str, development: bool) -> Result<()> {
    use std::fs;
    use std::path::Path;
    
    println!("生成配置文件到: {}", output_path);
    
    let content = if development {
        include_str!("../config/development.toml")
    } else {
        include_str!("../config/default.toml")
    };
    
    // 确保目录存在
    let path = Path::new(output_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            rusher::error::RusherError::IoError(e)
        })?;
    }
    
    // 写入文件
    fs::write(path, content).map_err(|e| {
        rusher::error::RusherError::IoError(e)
    })?;
    
    println!("配置文件生成成功!");
    
    Ok(())
}

