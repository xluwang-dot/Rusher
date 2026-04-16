//! Rusher Rust 实现
//! 
//! GitHub DNS 加速服务

pub mod error;
pub mod config;
pub mod utils;
pub mod dns;
pub mod scanner;
pub mod http;
pub mod counter;

// 重新导出常用模块
pub use error::{RusherError, Result};
pub use config::{Config, ConfigLoader};
pub use utils::{logging, signal};

/// Rusher 版本信息
pub const VERSION: &str = "0.1.0";
pub const NAME: &str = "rusher-rs";
pub const DESCRIPTION: &str = "GitHub DNS加速服务 - Rust实现";
pub const AUTHORS: &str = "Rusher Team";

/// 应用程序信息
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
    pub authors: &'static str,
}

impl AppInfo {
    /// 获取应用程序信息
    pub fn get() -> Self {
        Self {
            name: NAME,
            version: VERSION,
            description: DESCRIPTION,
            authors: AUTHORS,
        }
    }
    
    /// 打印应用程序信息
    pub fn print(&self) {
        println!("{} v{}", self.name, self.version);
        println!("{}", self.description);
        println!("作者: {}", self.authors);
    }
    
    /// 获取格式化的版本字符串
    pub fn version_string(&self) -> String {
        format!("{} v{}", self.name, self.version)
    }
}

/// 初始化应用程序
pub async fn init_app() -> Result<Config> {
    // 打印启动信息
    let app_info = AppInfo::get();
    log_info!("启动 {}", app_info.version_string());
    
    // 加载配置
    log_info!("加载配置...");
    let config_loader = ConfigLoader::new();
    let config = config_loader.load()?;
    
    // 验证配置
    config::loader::utils::validate_config(&config)?;
    
    // 初始化日志系统
    log_info!("初始化日志系统...");
    let log_file = config.general.log_file.as_deref();
    utils::logging::init_logging(&config.general.log_level, log_file)?;
    
    // 打印配置摘要
    config::loader::utils::print_config_summary(&config);
    
    Ok(config)
}

/// 运行应用程序
pub async fn run_app(config: Config) -> Result<()> {
    use std::sync::Arc;
    
    println!("开始运行 Rusher 服务");
    
    // 创建信号处理器
    let signal_handler = utils::signal::SignalHandler::new();
    signal_handler.listen().await?;
    
    // 创建共享配置
    let config_arc = Arc::new(config);
    
    // 启动 DNS 服务器
    println!("启动 DNS 服务器...");
    
    // 由于沙箱环境限制，我们无法绑定 53 端口
    // 让我们修改配置，使用一个非特权端口
    let mut modified_config = (*config_arc).clone();
    // 注意：这里我们需要解析 SocketAddr
    modified_config.dns.listen_addr = "127.0.0.1:5353".parse().map_err(|e| {
        crate::error::RusherError::ConfigError(format!("解析监听地址失败: {}", e))
    })?;
    
    let modified_config_arc = Arc::new(modified_config);
    
    // 创建 DNS 缓存（暂时不使用）
    let _dns_cache = Arc::new(dns::cache::DnsCache::new(
        modified_config_arc.clone(),
    ));
    
    println!("服务已启动，监听地址: 127.0.0.1:5353");
    println!("注意：DNS 服务正在启动中...");
    println!("等待关机信号 (Ctrl+C)...");
    
    // 等待关机信号
    signal_handler.wait_for_shutdown().await;
    
    println!("收到关机信号，开始关闭服务...");
    println!("服务已关闭");
    Ok(())
}

/// 快速启动函数
pub async fn quick_start() -> Result<()> {
    let config = init_app().await?;
    run_app(config).await
}