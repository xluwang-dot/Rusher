//! 配置数据结构定义

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;

/// 通用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// 日志级别
    pub log_level: String,
    
    /// 日志文件路径
    pub log_file: Option<String>,
    
    /// 是否以守护进程模式运行
    pub daemon: bool,
}

/// DNS 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    /// DNS 监听地址
    pub listen_addr: SocketAddr,
    
    /// DNS 缓存生存时间（秒）
    pub cache_ttl: u32,
    
    /// 是否启用 IPv6
    pub enable_ipv6: bool,
    
    /// 上游 DNS 服务器
    pub upstream_dns: Vec<SocketAddr>,
    
    /// 是否启用 DNS over HTTPS
    pub doh_enabled: bool,
    
    /// DNS over HTTPS 端点
    pub doh_endpoint: Option<String>,
}

/// 扫描器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannerConfig {
    /// 全量扫描间隔（秒）
    pub scan_interval: u64,
    
    /// 扫描超时时间（秒）
    pub scan_timeout: u64,
    
    /// 最大并发连接数
    pub max_concurrent: usize,
    
    /// 重试次数
    pub retry_count: u32,
    
    /// 连接超时（秒）
    pub connect_timeout: u64,
    
    /// 请求超时（秒）
    pub request_timeout: u64,
    
    /// 是否启用增量扫描
    pub incremental_scan: bool,
    
    /// 增量扫描间隔（秒）
    pub incremental_interval: u64,
}

/// GitHub 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubConfig {
    /// GitHub Meta API 地址
    pub meta_url: String,
    
    /// 需要加速的 GitHub 域名
    pub domains: Vec<String>,
    
    /// 自定义 IP 范围（CIDR格式）
    pub custom_ranges: Vec<String>,
    
    /// 是否启用 GitHub API 认证
    pub api_auth_enabled: bool,
    
    /// GitHub API Token
    pub api_token: Option<String>,
}

/// HTTP 客户端配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// User-Agent 头
    pub user_agent: String,
    
    /// 连接池大小
    pub connection_pool_size: usize,
    
    /// 是否启用 HTTP/2
    pub http2_enabled: bool,
    
    /// 是否启用压缩
    pub compression_enabled: bool,
    
    /// 代理设置
    pub proxy: Option<String>,
}

/// 缓存配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// 扫描结果缓存大小
    pub scan_cache_size: u64,
    
    /// DNS 缓存大小
    pub dns_cache_size: u64,
    
    /// 缓存过期时间（秒）
    pub cache_expiry: u64,
}

impl CacheConfig {
    /// 获取缓存过期时间
    pub fn cache_expiry_duration(&self) -> Duration {
        Duration::from_secs(self.cache_expiry)
    }
}

/// 监控配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// 是否启用监控
    pub enabled: bool,
    
    /// 监控端口
    pub port: u16,
    
    /// 监控端点路径
    pub path: String,
    
    /// 是否启用健康检查
    pub health_check: bool,
}

/// 完整的配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 通用配置
    pub general: GeneralConfig,
    
    /// DNS 配置
    pub dns: DnsConfig,
    
    /// 扫描器配置
    pub scanner: ScannerConfig,
    
    /// GitHub 配置
    pub github: GithubConfig,
    
    /// HTTP 配置
    pub http: HttpConfig,
    
    /// 缓存配置
    pub cache: CacheConfig,
    
    /// 监控配置
    pub monitoring: MonitoringConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                log_level: "info".to_string(),
                log_file: Some("rusher.log".to_string()),
                daemon: false,
            },
            dns: DnsConfig {
                listen_addr: "0.0.0.0:53".parse().unwrap(),
                cache_ttl: 300,
                enable_ipv6: false,
                upstream_dns: vec![
                    "8.8.8.8:53".parse().unwrap(),
                    "1.1.1.1:53".parse().unwrap(),
                ],
                doh_enabled: false,
                doh_endpoint: Some("https://cloudflare-dns.com/dns-query".to_string()),
            },
            scanner: ScannerConfig {
                scan_interval: 300,
                scan_timeout: 10,
                max_concurrent: 100,
                retry_count: 3,
                connect_timeout: 5,
                request_timeout: 10,
                incremental_scan: true,
                incremental_interval: 60,
            },
            github: GithubConfig {
                meta_url: "https://api.github.com/meta".to_string(),
                domains: vec![
                    "github.com".to_string(),
                    "github.global.ssl.fastly.net".to_string(),
                    "raw.githubusercontent.com".to_string(),
                    "gist.github.com".to_string(),
                    "github.io".to_string(),
                    "githubusercontent.com".to_string(),
                    "githubassets.com".to_string(),
                    "github.dev".to_string(),
                ],
                custom_ranges: vec![],
                api_auth_enabled: false,
                api_token: None,
            },
            http: HttpConfig {
                user_agent: "Rusher-Rust/1.0".to_string(),
                connection_pool_size: 100,
                http2_enabled: true,
                compression_enabled: true,
                proxy: None,
            },
            cache: CacheConfig {
                scan_cache_size: 10000,
                dns_cache_size: 1000,
                cache_expiry: 3600,
            },
            monitoring: MonitoringConfig {
                enabled: false,
                port: 9090,
                path: "/metrics".to_string(),
                health_check: true,
            },
        }
    }
}

impl Config {
    /// 获取扫描间隔的 Duration
    pub fn scan_interval_duration(&self) -> Duration {
        Duration::from_secs(self.scanner.scan_interval)
    }
    
    /// 获取扫描超时的 Duration
    pub fn scan_timeout_duration(&self) -> Duration {
        Duration::from_secs(self.scanner.scan_timeout)
    }
    
    /// 获取连接超时的 Duration
    pub fn connect_timeout_duration(&self) -> Duration {
        Duration::from_secs(self.scanner.connect_timeout)
    }
    
    /// 获取请求超时的 Duration
    pub fn request_timeout_duration(&self) -> Duration {
        Duration::from_secs(self.scanner.request_timeout)
    }
    
    /// 获取增量扫描间隔的 Duration
    pub fn incremental_interval_duration(&self) -> Duration {
        Duration::from_secs(self.scanner.incremental_interval)
    }
    
    /// 获取缓存过期时间的 Duration
    pub fn cache_expiry_duration(&self) -> Duration {
        Duration::from_secs(self.cache.cache_expiry)
    }
}