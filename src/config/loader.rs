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
        
        let config = ConfigBuilder::builder()
            .add_source(File::from(path))
            .build()
            .map_err(|e| RusherError::ConfigError(format!("配置构建失败: {}", e)))?;
        
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