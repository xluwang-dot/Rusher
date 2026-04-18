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
pub use http::HttpClient;
pub use scanner::{IpScanner, GithubApiClient};
pub use dns::{DnsServer, DnsCache, DnsResolver};

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
    
    // 创建 HTTP 客户端
    let http_client = Arc::new(http::HttpClient::new(config_arc.clone())?);
    
    // 创建 GitHub API 客户端
    let github_client = Arc::new(scanner::github::GithubApiClient::new(config_arc.clone())?);
    
    // 创建扫描缓存
    let scan_cache = Arc::new(scanner::cache::ScanCache::new(config_arc.clone()));
    
    // 创建 IP 扫描器
    let scanner = Arc::new(scanner::IpScanner::new(
        config_arc.clone(),
        scan_cache,
        github_client,
        http_client,
    ));
    
    // 启动扫描器
    scanner.start().await?;
    
    // 等待一段时间让扫描器完成初始扫描
    println!("等待扫描器完成初始扫描...");
    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    
    // 打印最优 IP 地址
    println!("\n=== 扫描结果 ===");
    scanner.print_optimal_ips().await;
    
    // 打印扫描统计信息
    let stats = scanner.get_stats().await;
    stats.print();
    
    // 创建 DNS 缓存
    let dns_cache = Arc::new(dns::cache::DnsCache::new(config_arc.clone()));
    
    // 创建 DNS 解析器
    let dns_resolver = Arc::new(dns::resolver::DnsResolver::new(
        config_arc.clone(),
        scanner.clone(),
    ));
    
    // 创建并启动 DNS 服务器
    let mut dns_server = dns::server::DnsServer::new(
        config_arc.clone(),
        dns_cache,
        dns_resolver,
    );
    
    // 在后台启动 DNS 服务器
    let dns_server_handle = tokio::spawn(async move {
        if let Err(e) = dns_server.start().await {
            eprintln!("DNS 服务器启动失败: {}", e);
        }
    });
    
    // 显示实际的监听地址
    println!("服务已启动，监听地址: {}", config_arc.dns.listen_addr);
    println!("等待关机信号 (Ctrl+C)...");
    
    // 等待关机信号
    signal_handler.wait_for_shutdown().await;
    
    println!("收到关机信号，开始关闭服务...");
    
    // 停止扫描器
    scanner.stop().await?;
    
    // 等待 DNS 服务器停止
    dns_server_handle.abort();
    
    println!("服务已关闭");
    Ok(())
}

/// 快速启动函数
pub async fn quick_start() -> Result<()> {
    let config = init_app().await?;
    run_app(config).await
}

/// 快速启动函数（带配置文件）
pub async fn quick_start_with_config(config_path: Option<String>) -> Result<()> {
    let config = if let Some(path) = config_path {
        ConfigLoader::load_from_path(&path)?
    } else {
        init_app().await?
    };
    run_app(config).await
}