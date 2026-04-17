//! IP 扫描器实现

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio::time;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::error::{RusherError, Result};
use crate::http::HttpClient;
use super::cache::ScanCache;
use super::github::GithubApiClient;

/// IP 扫描结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    /// IP 地址
    pub ip: IpAddr,
    /// 域名
    pub domain: String,
    /// 响应时间（毫秒）
    pub response_time: u64,
    /// 扫描时间（Unix 时间戳，毫秒）
    pub scanned_at_ms: u128,
    /// 是否可用
    pub available: bool,
}

/// IP 扫描器
pub struct IpScanner {
    /// 配置
    config: Arc<Config>,
    /// 扫描缓存
    cache: Arc<ScanCache>,
    /// GitHub API 客户端
    github_client: Arc<GithubApiClient>,
    /// HTTP 客户端
    http_client: Arc<dyn crate::http::HttpClientTrait + Send + Sync>,
    /// 扫描结果（域名 -> IP -> 扫描结果）
    results: RwLock<HashMap<String, HashMap<IpAddr, ScanResult>>>,
    /// 最快的 IP 地址（域名 -> IP）
    fastest_ips: RwLock<HashMap<String, IpAddr>>,
    /// 扫描任务句柄
    scan_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl IpScanner {
    /// 创建一个空的扫描器（用于测试和占位）
    pub fn empty() -> Self {
        use std::collections::HashMap;
        
        use std::sync::Arc;
        
        // 创建一个默认配置
        let config = Arc::new(Config::default());
        
        // 创建简单的客户端（不调用 empty() 方法）
        let github_client = Arc::new(GithubApiClient::new(config.clone()).expect("Failed to create GithubApiClient"));
        
        // 创建一个简单的 HttpClient 占位
        // 由于 HttpClient 有私有字段，我们无法直接创建
        // 这里我们使用一个简单的包装
        struct SimpleHttpClient;
        
        impl SimpleHttpClient {
            async fn test_ip_domain(&self, _ip: &str, _domain: &str, _use_https: bool) -> crate::http::Result<std::time::Duration> {
                // 返回一个默认的延迟
                Ok(std::time::Duration::from_millis(100))
            }
        }
        
        // 创建一个包装器
        struct HttpClientWrapper {
            inner: SimpleHttpClient,
        }
        
        // 实现 HttpClientTrait
        impl crate::http::HttpClientTrait for HttpClientWrapper {
            fn test_ip_domain(&self, ip: &str, domain: &str, use_https: bool) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::http::Result<std::time::Duration>> + Send + '_>> {
                let ip = ip.to_string();
                let domain = domain.to_string();
                Box::pin(async move {
                    self.inner.test_ip_domain(&ip, &domain, use_https).await
                })
            }
        }
        
        // 由于我们无法直接创建 HttpClient，这里使用一个占位
        // 在实际使用中，应该正确处理 HttpClient 的创建错误
        // 这里我们返回一个简单的占位实现
        // 注意：这只是一个占位实现，实际使用时应该确保 HttpClient::new 成功
        let http_client = Arc::new(HttpClientWrapper {
            inner: SimpleHttpClient,
        });
        
        Self {
            config: config.clone(),
            cache: Arc::new(ScanCache::new(config.clone())),
            github_client,
            http_client,
            results: RwLock::new(HashMap::new()),
            fastest_ips: RwLock::new(HashMap::new()),
            scan_handle: Mutex::new(None),
        }
    }
    
    /// 创建新的 IP 扫描器
    pub fn new(
        config: Arc<Config>,
        cache: Arc<ScanCache>,
        github_client: Arc<GithubApiClient>,
        http_client: Arc<dyn crate::http::HttpClientTrait + Send + Sync>,
    ) -> Self {
        Self {
            config,
            cache,
            github_client,
            http_client,
            results: RwLock::new(HashMap::new()),
            fastest_ips: RwLock::new(HashMap::new()),
            scan_handle: Mutex::new(None),
        }
    }

    /// 启动扫描器
    pub async fn start(&self) -> Result<()> {
        info!("启动 IP 扫描器");
        
        // 加载初始 IP 范围
        self.load_ip_ranges().await?;
        
        // 启动扫描任务
        let _config = self.config.clone();
        let scanner = Arc::new(self.clone());
        
        let handle = tokio::spawn(async move {
            scanner.run_scan_loop().await;
        });
        
        *self.scan_handle.lock().await = Some(handle);
        
        Ok(())
    }

    /// 停止扫描器
    pub async fn stop(&self) -> Result<()> {
        info!("停止 IP 扫描器");
        
        let mut handle_guard = self.scan_handle.lock().await;
        if let Some(handle) = handle_guard.take() {
            handle.abort();
        }
        
        Ok(())
    }

    /// 运行扫描循环
    async fn run_scan_loop(self: Arc<Self>) {
        info!("开始扫描循环");
        
        loop {
            // 执行全量扫描
            if let Err(e) = self.full_scan().await {
                error!("全量扫描失败: {}", e);
            }
            
            // 等待下一次扫描
            let scan_interval = self.config.scan_interval_duration();
            time::sleep(scan_interval).await;
        }
    }

    /// 执行全量扫描
    async fn full_scan(&self) -> Result<()> {
        info!("开始全量扫描");
        
        // 获取所有需要扫描的域名
        let domains = self.config.github.domains.clone();
        
        for domain in domains {
            debug!("扫描域名: {}", domain);
            
            // 获取该域名的 IP 地址列表
            let ips = self.get_ips_for_domain(&domain).await?;
            
            // 并发扫描所有 IP
            let scan_results = self.scan_ips(&domain, &ips).await;
            
            // 更新扫描结果
            self.update_results(&domain, scan_results).await;
            
            // 更新最快的 IP
            self.update_fastest_ip(&domain).await;
        }
        
        info!("全量扫描完成");
        Ok(())
    }

    /// 获取域名的 IP 地址列表
    async fn get_ips_for_domain(&self, domain: &str) -> Result<Vec<IpAddr>> {
        debug!("获取域名 {} 的 IP 地址列表", domain);
        
        // 首先检查缓存
        if let Some(ips) = self.cache.get_ips_for_domain(domain) {
            debug!("从缓存获取 IP 地址列表: {} 个 IP", ips.len());
            return Ok(ips);
        }
        
        // 从 GitHub API 获取 IP 范围
        let _ip_ranges = self.github_client.get_ip_ranges().await?;
        
        // 提取该域名的 IP 地址
        let ips = Vec::new();
        
        // 这里应该根据域名从 IP 范围中提取对应的 IP
        // 暂时返回空列表
        warn!("IP 地址提取功能尚未实现: {}", domain);
        
        // 更新缓存
        self.cache.set_ips_for_domain(domain, ips.clone());
        
        Ok(ips)
    }

    /// 扫描 IP 地址
    async fn scan_ips(&self, domain: &str, ips: &[IpAddr]) -> Vec<ScanResult> {
        debug!("扫描 {} 个 IP 地址", ips.len());
        
        let mut results = Vec::new();
        
        // 简单实现：顺序扫描（后续可以优化为并发扫描）
        for &ip in ips {
            let scan_result = self.scan_ip(domain, ip).await;
            results.push(scan_result);
        }
        
        results
    }

    /// 扫描单个 IP 地址
    async fn scan_ip(&self, domain: &str, ip: IpAddr) -> ScanResult {
        debug!("扫描 IP: {} -> {}", domain, ip);
        
        let _start_time = Instant::now();
        let mut available = false;
        let mut response_time = u64::MAX;
        
        // 尝试连接
        match self.test_connection(domain, ip).await {
            Ok(duration) => {
                available = true;
                response_time = duration.as_millis() as u64;
                debug!("IP {} 可用，响应时间: {}ms", ip, response_time);
            }
            Err(e) => {
                warn!("IP {} 不可用: {}", ip, e);
            }
        }
        
        ScanResult {
            ip,
            domain: domain.to_string(),
            response_time,
            scanned_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            available,
        }
    }

    /// 测试连接
    async fn test_connection(&self, domain: &str, ip: IpAddr) -> Result<Duration> {
        debug!("测试连接: {} -> {}", domain, ip);
        
        // 将 IP 地址转换为字符串
        let ip_str = ip.to_string();
        
        // 对于 GitHub 域名，我们通常使用 HTTPS
        let use_https = true;
        
        // 使用 HTTP 客户端测试连接
        match self.http_client.test_ip_domain(&ip_str, domain, use_https).await {
            Ok(duration) => {
                debug!("连接测试成功: {} -> {} ({}ms)", ip, domain, duration.as_millis());
                Ok(duration)
            }
            Err(e) => {
                // 如果 HTTPS 失败，尝试 HTTP
                if use_https {
                    debug!("HTTPS 连接失败，尝试 HTTP: {} -> {}: {}", ip, domain, e);
                    match self.http_client.test_ip_domain(&ip_str, domain, false).await {
                        Ok(duration) => {
                            debug!("HTTP 连接测试成功: {} -> {} ({}ms)", ip, domain, duration.as_millis());
                            Ok(duration)
                        }
                        Err(e2) => {
                            warn!("连接测试失败: {} -> {}: {}", ip, domain, e2);
                            Err(RusherError::NetworkError(format!(
                                "连接测试失败: {} -> {}: {}",
                                ip, domain, e2
                            )))
                        }
                    }
                } else {
                    warn!("连接测试失败: {} -> {}: {}", ip, domain, e);
                    Err(RusherError::NetworkError(format!(
                        "连接测试失败: {} -> {}: {}",
                        ip, domain, e
                    )))
                }
            }
        }
    }

    /// 更新扫描结果
    async fn update_results(&self, domain: &str, new_results: Vec<ScanResult>) {
        let mut results_guard = self.results.write().await;
        
        let domain_results = results_guard
            .entry(domain.to_string())
            .or_insert_with(HashMap::new);
        
        for result in new_results {
            domain_results.insert(result.ip, result);
        }
        
        // 更新缓存
        self.cache.set_scan_results(domain, domain_results.values().cloned().collect());
    }

    /// 更新最快的 IP
    async fn update_fastest_ip(&self, domain: &str) {
        let results_guard = self.results.read().await;
        
        if let Some(domain_results) = results_guard.get(domain) {
            // 找到可用的、响应时间最短的 IP
            let fastest = domain_results
                .values()
                .filter(|result| result.available)
                .min_by_key(|result| result.response_time);
            
            if let Some(fastest_result) = fastest {
                let mut fastest_ips_guard = self.fastest_ips.write().await;
                fastest_ips_guard.insert(domain.to_string(), fastest_result.ip);
                
                debug!("更新最快的 IP: {} -> {}", domain, fastest_result.ip);
            }
        }
    }

    /// 加载 IP 范围
    async fn load_ip_ranges(&self) -> Result<()> {
        info!("加载 IP 范围");
        
        // 从 GitHub API 获取 IP 范围
        let ip_ranges = self.github_client.get_ip_ranges().await?;
        
        // 处理 IP 范围
        self.process_ip_ranges(ip_ranges).await;
        
        Ok(())
    }

    /// 处理 IP 范围
    async fn process_ip_ranges(&self, ip_ranges: Vec<String>) {
        info!("处理 {} 个 IP 范围", ip_ranges.len());
        
        // 这里应该实现 IP 范围处理逻辑
        // 暂时只记录日志
        for range in ip_ranges {
            debug!("IP 范围: {}", range);
        }
    }

    /// 获取最快的 IPv4 地址
    pub async fn get_fastest_ipv4(&self, domain: &str) -> Option<Ipv4Addr> {
        let fastest_ips_guard = self.fastest_ips.read().await;
        
        if let Some(ip) = fastest_ips_guard.get(domain) {
            if let IpAddr::V4(ipv4) = ip {
                return Some(*ipv4);
            }
        }
        
        None
    }

    /// 获取最快的 IPv6 地址
    pub async fn get_fastest_ipv6(&self, domain: &str) -> Option<Ipv6Addr> {
        let fastest_ips_guard = self.fastest_ips.read().await;
        
        if let Some(ip) = fastest_ips_guard.get(domain) {
            if let IpAddr::V6(ipv6) = ip {
                return Some(*ipv6);
            }
        }
        
        None
    }

    /// 获取扫描统计信息
    pub async fn get_stats(&self) -> ScanStats {
        let results_guard = self.results.read().await;
        let fastest_ips_guard = self.fastest_ips.read().await;
        
        let mut total_ips = 0;
        let mut available_ips = 0;
        
        for domain_results in results_guard.values() {
            total_ips += domain_results.len();
            available_ips += domain_results.values().filter(|r| r.available).count();
        }
        
        ScanStats {
            total_domains: results_guard.len(),
            total_ips,
            available_ips,
            fastest_ips_count: fastest_ips_guard.len(),
        }
    }
}

/// 扫描统计信息
#[derive(Debug, Clone)]
pub struct ScanStats {
    /// 总域名数量
    pub total_domains: usize,
    /// 总 IP 数量
    pub total_ips: usize,
    /// 可用 IP 数量
    pub available_ips: usize,
    /// 最快的 IP 数量
    pub fastest_ips_count: usize,
}

impl ScanStats {
    /// 打印统计信息
    pub fn print(&self) {
        println!("IP 扫描统计:");
        println!("  域名数量: {}", self.total_domains);
        println!("  IP 数量: {}", self.total_ips);
        println!("  可用 IP: {}", self.available_ips);
        println!("  最快的 IP: {}", self.fastest_ips_count);
        
        if self.total_ips > 0 {
            let availability_rate = (self.available_ips as f64 / self.total_ips as f64) * 100.0;
            println!("  可用率: {:.2}%", availability_rate);
        }
    }
}

impl Clone for IpScanner {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            cache: self.cache.clone(),
            github_client: self.github_client.clone(),
            http_client: self.http_client.clone(),
            results: RwLock::new(HashMap::new()),
            fastest_ips: RwLock::new(HashMap::new()),
            scan_handle: Mutex::new(None),
        }
    }
}