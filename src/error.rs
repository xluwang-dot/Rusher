//! 错误处理模块
//! 
//! 定义 Rusher 项目的错误类型和错误处理

use thiserror::Error;

/// Rusher 项目错误类型
#[derive(Error, Debug)]
pub enum RusherError {
    /// 配置错误
    #[error("配置错误: {0}")]
    ConfigError(String),
    
    /// IO 错误
    #[error("IO 错误: {0}")]
    IoError(#[from] std::io::Error),
    
    /// 网络错误
    #[error("网络错误: {0}")]
    NetworkError(String),
    
    /// DNS 错误
    #[error("DNS 错误: {0}")]
    DnsError(String),
    
    /// HTTP 错误
    #[error("HTTP 错误: {0}")]
    HttpError(String),
    
    /// 解析错误
    #[error("解析错误: {0}")]
    ParseError(String),
    
    /// URL 解析错误
    #[error("URL 解析错误: {0}")]
    UrlParseError(#[from] url::ParseError),
    
    /// 扫描错误
    #[error("扫描错误: {0}")]
    ScanError(String),
    
    /// 缓存错误
    #[error("缓存错误: {0}")]
    CacheError(String),
    
    /// 系统错误
    #[error("系统错误: {0}")]
    SystemError(String),
    
    /// 未知错误
    #[error("未知错误: {0}")]
    UnknownError(String),
}

/// 结果类型别名
pub type Result<T> = std::result::Result<T, RusherError>;

/// 错误转换工具
pub trait ErrorExt<T> {
    /// 将错误转换为 RusherError
    fn with_context<F, S>(self, context: F) -> Result<T>
    where
        F: FnOnce() -> S,
        S: Into<String>;
}

impl<T, E> ErrorExt<T> for std::result::Result<T, E>
where
    E: std::error::Error,
{
    fn with_context<F, S>(self, context: F) -> Result<T>
    where
        F: FnOnce() -> S,
        S: Into<String>,
    {
        self.map_err(|e| RusherError::UnknownError(format!("{}: {}", context().into(), e)))
    }
}

/// 错误处理工具函数
pub mod utils {
    use super::*;
    
    /// 将任何错误转换为 RusherError
    pub fn to_rusher_error<E: std::error::Error>(error: E) -> RusherError {
        RusherError::UnknownError(error.to_string())
    }
    
    /// 创建配置错误
    pub fn config_error(msg: impl Into<String>) -> RusherError {
        RusherError::ConfigError(msg.into())
    }
    
    /// 创建网络错误
    pub fn network_error(msg: impl Into<String>) -> RusherError {
        RusherError::NetworkError(msg.into())
    }
    
    /// 创建 DNS 错误
    pub fn dns_error(msg: impl Into<String>) -> RusherError {
        RusherError::DnsError(msg.into())
    }
}