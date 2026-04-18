//! 配置加载器
//! 
//! 负责从文件、环境变量等加载配置

use crate::error::{RusherError, Result};
use crate::config::models::Config;
use config::{Config as ConfigBuilder, File, FileFormat, Environment};
use std::path::Path;

/// 配置加载器
pub struct ConfigLoader {
    config_path: Option<String>,
    env_prefix: String,
}

impl ConfigLoader {
    /// 创建新的配置加载器
    pub fn new() -> Self {
        Self {
            config_path: None,
            env_prefix: "FASTGITHUB".to_string(),
        }
    }
    
    /// 设置配置文件路径
    pub fn with_config_path(mut self, path: impl Into<String>) -> Self {
        self.config_path = Some(path.into());
        self
    }
    
    /// 设置环境变量前缀
    pub fn with_env_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.env_prefix = prefix.into();
        self
    }
    
    /// 加载配置
    pub fn load(&self) -> Result<Config> {
        let mut builder = ConfigBuilder::builder();
        
        // 添加默认值 - 使用 set_default 而不是 add_source
        // 首先，我们需要将默认配置转换为可以设置的值
        // 我们可以通过序列化为字符串再反序列化的方式
        // let default_config = Config::default();
        
        // 添加配置文件
        if let Some(ref path) = self.config_path {
            if Path::new(path).exists() {
                builder = builder.add_source(File::new(path, FileFormat::Toml));
            } else {
                return Err(RusherError::ConfigError(format!(
                    "配置文件不存在: {}",
                    path
                )));
            }
        } else {
            // 尝试从默认位置加载配置
            let default_paths = vec![
                "./config/default.toml",
                "./config/development.toml",
                "/etc/rusher/config.toml",
            ];
            
            for path in default_paths {
                if Path::new(path).exists() {
                    builder = builder.add_source(File::new(path, FileFormat::Toml));
                    break;
                }
            }
        }
        
        // 添加环境变量
        builder = builder.add_source(
            Environment::with_prefix(&self.env_prefix)
                .separator("__")
                .try_parsing(true)
        );
        
        // 构建配置
        let config = builder.build()
            .map_err(|e| RusherError::ConfigError(format!("配置构建失败: {}", e)))?;
        
        // 反序列化为 Config 结构
        let loaded_config: Config = config.try_deserialize()
            .map_err(|e| RusherError::ConfigError(format!("配置反序列化失败: {}", e)))?;
        
        // 合并默认值（对于缺失的字段）
        Ok(loaded_config)
    }
    
    /// 从指定路径加载配置
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Config> {
        let path = path.as_ref();
        
        if !path.exists() {
            return Err(RusherError::ConfigError(format!(
                "配置文件不存在: {}",
                path.display()
            )));
        }
        
        // 首先加载默认配置
        let default_config = Config::default();
        
        // 然后加载指定配置文件
        let config = ConfigBuilder::builder()
            // 添加默认值
            .set_default("general.log_level", default_config.general.log_level.to_string())?
            .set_default("general.log_file", default_config.general.log_file.unwrap_or_default())?
            .set_default("general.daemon", default_config.general.daemon)?
            .set_default("dns.listen_addr", default_config.dns.listen_addr.to_string())?
            .set_default("dns.cache_ttl", default_config.dns.cache_ttl as i64)?
            .set_default("dns.enable_ipv6", default_config.dns.enable_ipv6)?
            .set_default("dns.upstream_dns", default_config.dns.upstream_dns.iter().map(|addr| addr.to_string()).collect::<Vec<_>>())?
            .set_default("dns.doh_enabled", default_config.dns.doh_enabled)?
            .set_default("dns.doh_endpoint", default_config.dns.doh_endpoint)?
            .set_default("scanner.scan_interval", default_config.scanner.scan_interval as i64)?
            .set_default("scanner.scan_timeout", default_config.scanner.scan_timeout as i64)?
            .set_default("scanner.max_concurrent", default_config.scanner.max_concurrent as i64)?
            .set_default("scanner.retry_count", default_config.scanner.retry_count as i64)?
            .set_default("scanner.connect_timeout", default_config.scanner.connect_timeout as i64)?
            .set_default("scanner.request_timeout", default_config.scanner.request_timeout as i64)?
            .set_default("scanner.incremental_scan", default_config.scanner.incremental_scan)?
            .set_default("scanner.incremental_interval", default_config.scanner.incremental_interval as i64)?
            .set_default("scanner.max_ips_per_cidr", default_config.scanner.max_ips_per_cidr as i64)?
            .set_default("github.meta_url", default_config.github.meta_url)?
            .set_default("github.domains", default_config.github.domains)?
            .set_default("github.custom_ranges", Vec::<String>::new())?
            .set_default("github.api_auth_enabled", default_config.github.api_auth_enabled)?
            .set_default("github.api_token", "")?
            .set_default("http.user_agent", default_config.http.user_agent)?
            .set_default("http.connection_pool_size", default_config.http.connection_pool_size as i64)?
            .set_default("http.http2_enabled", default_config.http.http2_enabled)?
            .set_default("http.compression_enabled", default_config.http.compression_enabled)?
            .set_default("http.proxy", "")?
            .set_default("cache.scan_cache_size", default_config.cache.scan_cache_size as i64)?
            .set_default("cache.dns_cache_size", default_config.cache.dns_cache_size as i64)?
            .set_default("cache.cache_expiry", default_config.cache.cache_expiry as i64)?
            .set_default("monitoring.enabled", default_config.monitoring.enabled)?
            .set_default("monitoring.port", default_config.monitoring.port as i64)?
            .set_default("monitoring.path", default_config.monitoring.path)?
            .set_default("monitoring.health_check", default_config.monitoring.health_check)?
            // 然后添加配置文件
            .add_source(File::from(path))
            .build()?;
        
        config.try_deserialize()
            .map_err(|e| RusherError::ConfigError(format!("配置反序列化失败: {}", e)))
    }
    
    /// 从环境变量加载配置
    pub fn load_from_env(prefix: &str) -> Result<Config> {
        let config = ConfigBuilder::builder()
            .add_source(
                Environment::with_prefix(prefix)
                    .separator("__")
                    .try_parsing(true)
            )
            .build()
            .map_err(|e| RusherError::ConfigError(format!("配置构建失败: {}", e)))?;
        
        config.try_deserialize()
            .map_err(|e| RusherError::ConfigError(format!("配置反序列化失败: {}", e)))
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// 配置工具函数
pub mod utils {
    use super::*;
    
    /// 获取默认配置路径
    pub fn get_default_config_path() -> Option<String> {
        let paths = vec![
            "./config/default.toml".to_string(),
            "./config/development.toml".to_string(),
            "/etc/rusher/config.toml".to_string(),
        ];
        
        for path in paths {
            if Path::new(&path).exists() {
                return Some(path);
            }
        }
        
        None
    }
    
    /// 验证配置
    pub fn validate_config(config: &Config) -> Result<()> {
        // 验证日志级别
        let valid_log_levels = vec!["trace", "debug", "info", "warn", "error"];
        if !valid_log_levels.contains(&config.general.log_level.as_str()) {
            return Err(RusherError::ConfigError(format!(
                "无效的日志级别: {}，有效值: {:?}",
                config.general.log_level, valid_log_levels
            )));
        }
        
        // 验证扫描间隔
        if config.scanner.scan_interval == 0 {
            return Err(RusherError::ConfigError(
                "扫描间隔必须大于0".to_string()
            ));
        }
        
        // 验证并发数
        if config.scanner.max_concurrent == 0 {
            return Err(RusherError::ConfigError(
                "最大并发数必须大于0".to_string()
            ));
        }
        
        // 验证 GitHub 域名
        if config.github.domains.is_empty() {
            return Err(RusherError::ConfigError(
                "GitHub 域名列表不能为空".to_string()
            ));
        }
        
        // 验证监听地址
        if config.dns.listen_addr.port() == 0 {
            return Err(RusherError::ConfigError(
                "DNS 监听端口不能为0".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// 打印配置摘要
    pub fn print_config_summary(config: &Config) {
        println!("=== Rusher 配置摘要 ===");
        println!("日志级别: {}", config.general.log_level);
        println!("DNS 监听地址: {}", config.dns.listen_addr);
        println!("扫描间隔: {} 秒", config.scanner.scan_interval);
        println!("最大并发数: {}", config.scanner.max_concurrent);
        println!("GitHub 域名数量: {}", config.github.domains.len());
        println!("监控启用: {}", config.monitoring.enabled);
        println!("==========================");
    }
}