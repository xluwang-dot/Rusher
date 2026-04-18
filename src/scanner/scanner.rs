//! IP 扫描器实现

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio::time;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::error::{RusherError, Result};
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
    /// 并发扫描控制信号量
    scan_semaphore: Arc<tokio::sync::Semaphore>,
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
        
        // 创建信号量，限制同时进行的扫描任务数量
        let semaphore_permits = config.scanner.max_concurrent;
        let scan_semaphore = Arc::new(tokio::sync::Semaphore::new(semaphore_permits));
        
        Self {
            config: config.clone(),
            cache: Arc::new(ScanCache::new(config.clone())),
            github_client,
            http_client,
            results: RwLock::new(HashMap::new()),
            fastest_ips: RwLock::new(HashMap::new()),
            scan_handle: Mutex::new(None),
            scan_semaphore,
        }
    }
    
    /// 创建新的 IP 扫描器
    pub fn new(
        config: Arc<Config>,
        cache: Arc<ScanCache>,
        github_client: Arc<GithubApiClient>,
        http_client: Arc<dyn crate::http::HttpClientTrait + Send + Sync>,
    ) -> Self {
        // 创建信号量，限制同时进行的扫描任务数量
        let semaphore_permits = config.scanner.max_concurrent;
        let scan_semaphore = Arc::new(tokio::sync::Semaphore::new(semaphore_permits));
        
        Self {
            config,
            cache,
            github_client,
            http_client,
            results: RwLock::new(HashMap::new()),
            fastest_ips: RwLock::new(HashMap::new()),
            scan_handle: Mutex::new(None),
            scan_semaphore,
        }
    }

    /// 启动扫描器
    pub async fn start(self: &Arc<Self>) -> Result<()> {
        info!("启动 IP 扫描器");
        
        println!("正在加载初始 IP 范围...");
        
        // 加载初始 IP 范围
        match self.load_ip_ranges().await {
            Ok(_) => println!("成功加载 IP 范围"),
            Err(e) => {
                eprintln!("加载 IP 范围失败: {}", e);
                return Err(e);
            }
        }
        
        println!("启动扫描任务...");
        
        // 启动扫描任务（使用同一个 Arc，共享 results/fastest_ips）
        let scanner = Arc::clone(self);
        
        let handle = tokio::spawn(async move {
            scanner.run_scan_loop().await;
        });
        
        *self.scan_handle.lock().await = Some(handle);
        
        println!("IP 扫描器启动完成");
        
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
        
        const THRESHOLD_MS: u64 = 100;
        
        // 获取所有需要扫描的域名
        let domains = self.config.github.domains.clone();
        
        for domain in domains {
            debug!("扫描域名: {}", domain);
            println!("\n--- 域名: {} ---", domain);
            
            // 检查当前最快 IP 的响应时间
            let current_fastest = {
                let results_guard = self.results.read().await;
                let fastest_ips_guard = self.fastest_ips.read().await;
                
                if let Some(ip) = fastest_ips_guard.get(&domain) {
                    if let Some(domain_results) = results_guard.get(&domain) {
                        if let Some(result) = domain_results.get(ip) {
                            Some((result.ip, result.response_time, result.available))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            
            match current_fastest {
                Some((ip, time, available)) if available && time < THRESHOLD_MS => {
                    println!("当前最优 IP: {} ({}ms) — 响应 < {}ms，保持当前 IP", ip, time, THRESHOLD_MS);
                    println!("跳过扫描，继续使用当前 IP");
                    continue;
                }
                Some((ip, time, available)) if available => {
                    println!("当前最优 IP: {} ({}ms) — 响应 >= {}ms，开始扫描寻找更优 IP", ip, time, THRESHOLD_MS);
                }
                Some((ip, _time, _available)) => {
                    println!("当前最优 IP: {} — 不可用，开始扫描寻找更优 IP", ip);
                }
                None => {
                    println!("尚未找到最优 IP，开始扫描...");
                }
            }
            
            // 获取该域名的 IP 地址列表
            let ips = self.get_ips_for_domain(&domain).await?;
            
            // 并发扫描所有 IP，逐个显示结果，找到 < threshold 就停止
            let scan_results = self.scan_ips_with_progress(&domain, &ips, THRESHOLD_MS).await;
            
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
        
        println!("正在从 GitHub API 获取 IP 范围...");
        
        // 从 GitHub API 获取结构化 IP 范围
        let meta = self.github_client.get_ip_ranges_structured().await?;
        
        // 根据域名映射到正确的服务类别
        let ranges = match domain {
            "github.com" => {
                // github.com 使用 web + api
                let mut r = meta.web.clone();
                r.extend(meta.api.clone());
                r
            }
            "github.global.ssl.fastly.net" => meta.web.clone(),
            "raw.githubusercontent.com" => meta.web.clone(),
            "gist.github.com" => {
                let mut r = meta.web.clone();
                r.extend(meta.api.clone());
                r
            }
            "github.io" => meta.pages.clone(),
            "githubusercontent.com" => meta.web.clone(),
            "githubassets.com" => meta.web.clone(),
            "github.dev" => meta.web.clone(),
            _ => meta.web.clone(),
        };
        
        println!("获取到 {} 个相关 IP 范围 (域名: {})", ranges.len(), domain);
        
        // 从 CIDR 范围中提取 IP
        let mut ips = self.extract_ips_from_ranges(&ranges);
        
        // DNS 解析：获取当前正在使用的 IP
        let dns_ips = self.resolve_domain_ips(domain).await;
        if !dns_ips.is_empty() {
            println!("DNS 解析得到 {} 个 IP: {}", dns_ips.len(), 
                dns_ips.iter().map(|ip| ip.to_string()).collect::<Vec<_>>().join(", "));
            ips.extend(dns_ips);
            // 去重
            let mut unique = HashSet::new();
            ips.retain(|ip| unique.insert(*ip));
        }
        
        if ips.is_empty() {
            warn!("无法为域名 {} 提取 IP 地址，使用备用 IP", domain);
            let fallback = self.get_fallback_ips(domain);
            self.cache.set_ips_for_domain(domain, fallback.clone());
            return Ok(fallback);
        }
        
        println!("为域名 {} 提取到 {} 个 IP 地址", domain, ips.len());
        
        // 更新缓存
        self.cache.set_ips_for_domain(domain, ips.clone());
        
        Ok(ips)
    }
    
    /// DNS 解析获取当前正在使用的 IP
    async fn resolve_domain_ips(&self, domain: &str) -> Vec<IpAddr> {
        use std::net::ToSocketAddrs;
        use std::process::Command;
        
        let mut ips = Vec::new();
        
        // 方法1: 使用 nslookup 获取所有 IP
        if let Ok(output) = Command::new("nslookup")
            .arg(domain)
            .output()
        {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                let line = line.trim();
                if let Some(addr_str) = line.strip_prefix("Address:").or(line.strip_prefix("地址:")) {
                    let addr_str = addr_str.trim();
                    if let Ok(ip) = addr_str.parse::<IpAddr>() {
                        if !ips.contains(&ip) {
                            ips.push(ip);
                        }
                    }
                }
            }
        }
        
        // 方法2: 使用 dig 获取所有 IP
        if ips.is_empty() {
            if let Ok(output) = Command::new("dig")
                .arg("+short")
                .arg(domain)
                .output()
            {
                let output_str = String::from_utf8_lossy(&output.stdout);
                for line in output_str.lines() {
                    let line = line.trim();
                    if let Ok(ip) = line.parse::<IpAddr>() {
                        if !ips.contains(&ip) {
                            ips.push(ip);
                        }
                    }
                }
            }
        }
        
        // 方法3: 使用系统 DNS 解析
        if ips.is_empty() {
            let addr_str = format!("{}:443", domain);
            if let Ok(addrs) = addr_str.to_socket_addrs() {
                for addr in addrs {
                    let ip = addr.ip();
                    if !ips.contains(&ip) {
                        ips.push(ip);
                    }
                }
            }
        }
        
        if !ips.is_empty() {
            info!("DNS 解析 {} 得到 {} 个 IP", domain, ips.len());
        } else {
            warn!("DNS 解析 {} 失败", domain);
        }
        
        ips
    }
    
    /// 从 CIDR 范围列表中提取 IP 地址
    fn extract_ips_from_ranges(&self, ranges: &[String]) -> Vec<IpAddr> {
        let mut ips = Vec::new();
        let max_ips_to_extract = self.config.scanner.max_ips_per_cidr;
        
        for range in ranges {
            if let Ok(cidr) = range.parse::<cidr::IpCidr>() {
                let extracted = self.extract_ips_from_cidr(&cidr);
                ips.extend(extracted);
            } else if let Ok(ip) = range.parse::<IpAddr>() {
                ips.push(ip);
            }
            
            // 收集够了就停止
            if ips.len() >= max_ips_to_extract * 10 {
                break;
            }
        }
        
        // 去重
        let mut unique = HashSet::new();
        ips.retain(|ip| unique.insert(*ip));
        
        // 限制总数
        let max_per_domain = 100;
        if ips.len() > max_per_domain {
            ips.truncate(max_per_domain);
        }
        
        ips
    }
    
    /// 从 CIDR 范围中提取 IP 地址
    fn extract_ips_from_cidr(&self, cidr: &cidr::IpCidr) -> Vec<IpAddr> {
        let mut ips = Vec::new();
        
        // 限制提取的 IP 数量，避免提取整个大范围
        let max_ips_to_extract = self.config.scanner.max_ips_per_cidr;
        
        match cidr {
            cidr::IpCidr::V4(cidr_v4) => {
                // 获取第一个地址作为网络地址
                let first_address = cidr_v4.first_address();
                let network_length = cidr_v4.network_length();
                
                // 计算网络大小
                let host_bits = 32 - network_length;
                let total_hosts = 1u64 << host_bits;
                
                // 对于大范围，只提取少量 IP
                let ips_to_extract = if total_hosts > 256 {
                    // 大范围：只提取 2-3 个 IP
                    2
                } else if total_hosts > max_ips_to_extract as u64 {
                    max_ips_to_extract
                } else {
                    total_hosts as usize
                };
                
                // 将 IPv4 地址转换为 u32
                let first_addr_u32: u32 = first_address.into();
                
                // 提取前几个 IP 地址（跳过网络地址和广播地址）
                for i in 1..=ips_to_extract {
                    if let Some(ip) = first_addr_u32.checked_add(i as u32) {
                        ips.push(IpAddr::V4(Ipv4Addr::from(ip)));
                    }
                }
            }
            cidr::IpCidr::V6(cidr_v6) => {
                // 获取第一个地址作为网络地址
                let first_address = cidr_v6.first_address();
                let network_length = cidr_v6.network_length();
                
                // 计算网络大小
                let _host_bits = 128 - network_length;
                
                // IPv6 地址空间太大，只提取少量地址
                let ips_to_extract = 2; // IPv6: 只提取 2 个地址
                
                // 将 IPv6 地址转换为 u128
                let first_addr_u128: u128 = first_address.into();
                
                // 提取前几个 IP 地址
                for i in 1..=ips_to_extract {
                    if let Some(ip) = first_addr_u128.checked_add(i as u128) {
                        ips.push(IpAddr::V6(Ipv6Addr::from(ip)));
                    }
                }
            }
        }
        
        ips
    }
    
    /// 获取备用 IP 地址（当无法从 API 提取时使用）
    fn get_fallback_ips(&self, domain: &str) -> Vec<IpAddr> {
        let mut ips = Vec::new();
        
        // 根据域名返回已知的备用 IP 地址
        match domain {
            "github.com" => {
                // GitHub 主站 IP 地址
                ips.push(IpAddr::V4(Ipv4Addr::new(140, 82, 112, 3)));
                ips.push(IpAddr::V4(Ipv4Addr::new(140, 82, 113, 3)));
                ips.push(IpAddr::V4(Ipv4Addr::new(140, 82, 114, 3)));
                ips.push(IpAddr::V4(Ipv4Addr::new(140, 82, 115, 3)));
            }
            "github.global.ssl.fastly.net" => {
                // Fastly CDN IP 地址
                ips.push(IpAddr::V4(Ipv4Addr::new(151, 101, 1, 194)));
                ips.push(IpAddr::V4(Ipv4Addr::new(151, 101, 65, 194)));
                ips.push(IpAddr::V4(Ipv4Addr::new(151, 101, 129, 194)));
                ips.push(IpAddr::V4(Ipv4Addr::new(151, 101, 193, 194)));
            }
            "raw.githubusercontent.com" => {
                // GitHub raw content IP 地址
                ips.push(IpAddr::V4(Ipv4Addr::new(185, 199, 108, 133)));
                ips.push(IpAddr::V4(Ipv4Addr::new(185, 199, 109, 133)));
                ips.push(IpAddr::V4(Ipv4Addr::new(185, 199, 110, 133)));
                ips.push(IpAddr::V4(Ipv4Addr::new(185, 199, 111, 133)));
            }
            "gist.github.com" => {
                // Gist IP 地址
                ips.push(IpAddr::V4(Ipv4Addr::new(140, 82, 112, 4)));
                ips.push(IpAddr::V4(Ipv4Addr::new(140, 82, 113, 4)));
            }
            "github.io" => {
                // GitHub Pages IP 地址
                ips.push(IpAddr::V4(Ipv4Addr::new(185, 199, 108, 153)));
                ips.push(IpAddr::V4(Ipv4Addr::new(185, 199, 109, 153)));
                ips.push(IpAddr::V4(Ipv4Addr::new(185, 199, 110, 153)));
                ips.push(IpAddr::V4(Ipv4Addr::new(185, 199, 111, 153)));
            }
            _ => {
                // 默认 IP 地址
                ips.push(IpAddr::V4(Ipv4Addr::new(192, 30, 255, 113)));
                ips.push(IpAddr::V4(Ipv4Addr::new(192, 30, 255, 112)));
            }
        }
        
        info!("为域名 {} 使用 {} 个备用 IP 地址", domain, ips.len());
        ips
    }

    /// 扫描 IP 地址（带进度显示和提前停止）
    async fn scan_ips_with_progress(&self, domain: &str, ips: &[IpAddr], threshold_ms: u64) -> Vec<ScanResult> {
        debug!("扫描 {} 个 IP 地址", ips.len());
        println!("开始扫描 {} 个 IP，扫描 < {}ms 将自动停止...", ips.len(), threshold_ms);
        
        let mut results = Vec::new();
        
        for &ip in ips {
            let result = self.scan_ip(domain, ip).await;
            
            if result.available {
                let marker = if result.response_time < threshold_ms { "✓" } else { " " };
                println!("  {} IP: {} | {}ms", marker, result.ip, result.response_time);
                if result.response_time < threshold_ms {
                    results.push(result);
                    println!("找到 < {}ms 的 IP，停止扫描", threshold_ms);
                    break;
                }
            } else {
                println!("  x IP: {} | 不可用", result.ip);
            }
            
            results.push(result);
        }
        
        // 打印本次扫描汇总
        let available: Vec<_> = results.iter().filter(|r| r.available).collect();
        if !available.is_empty() {
            let fastest = available.iter().min_by_key(|r| r.response_time).unwrap();
            println!("本次扫描: {} 个可用 IP，最快: {} ({}ms)", available.len(), fastest.ip, fastest.response_time);
        } else {
            println!("本次扫描: 没有找到可用的 IP");
        }
        
        results
    }

    /// 扫描单个 IP 地址
    async fn scan_ip(&self, domain: &str, ip: IpAddr) -> ScanResult {
        debug!("扫描 IP: {} -> {}", domain, ip);
        
        // Acquire a semaphore permit to limit concurrent scans
        let _permit = self.scan_semaphore.acquire().await
            .expect("Semaphore should not be closed");
        
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
        
        println!("正在从 GitHub API 获取 IP 范围...");
        
        // 从 GitHub API 获取 IP 范围
        let ip_ranges = self.github_client.get_ip_ranges().await?;
        
        println!("成功获取到 {} 个 IP 范围", ip_ranges.len());
        
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

    /// 打印最优 IP 地址
    pub async fn print_optimal_ips(&self) {
        let fastest_ips_guard = self.fastest_ips.read().await;
        let results_guard = self.results.read().await;
        
        if fastest_ips_guard.is_empty() {
            println!("尚未找到最优 IP 地址");
            println!("正在从 GitHub API 获取 IP 范围并扫描...");
            return;
        }
        
        println!("=== 最优 IP 地址 ===");
        println!("以下是从 GitHub API 获取的 IP 范围中提取并测试的最优 IP 地址:");
        println!();
        
        for (domain, ip) in fastest_ips_guard.iter() {
            // 获取该域名的所有扫描结果
            if let Some(domain_results) = results_guard.get(domain) {
                if let Some(result) = domain_results.get(ip) {
                    println!("域名: {}", domain);
                    println!("  最优 IP: {}", ip);
                    println!("  响应时间: {}ms", result.response_time);
                    println!("  可用性: {}", if result.available { "可用" } else { "不可用" });
                    
                    // 打印所有可用的 IP 地址及其响应时间
                    let mut available_ips: Vec<&ScanResult> = domain_results.values()
                        .filter(|r| r.available)
                        .collect();
                    
                    // 按响应时间排序
                    available_ips.sort_by_key(|r| r.response_time);
                    
                    if available_ips.len() > 1 {
                        println!("  所有可用 IP (按响应时间排序):");
                        for (i, result) in available_ips.iter().enumerate() {
                            let marker = if result.ip == *ip { "✓" } else { " " };
                            println!("    {}{}. {} ({}ms)", marker, i + 1, result.ip, result.response_time);
                        }
                    }
                    println!();
                }
            }
        }
        
        // 打印统计信息
        let stats = self.get_stats().await;
        println!("=== 扫描统计 ===");
        println!("  域名数量: {}", stats.total_domains);
        println!("  IP 数量: {}", stats.total_ips);
        println!("  可用 IP: {}", stats.available_ips);
        if stats.total_ips > 0 {
            let availability_rate = (stats.available_ips as f64 / stats.total_ips as f64) * 100.0;
            println!("  可用率: {:.2}%", availability_rate);
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
            scan_semaphore: self.scan_semaphore.clone(),
        }
    }
}