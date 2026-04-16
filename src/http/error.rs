//! HTTP 错误处理

use thiserror::Error;

/// HTTP 错误类型
#[derive(Debug, Error)]
pub enum HttpError {
    /// 网络错误
    #[error("网络错误: {0}")]
    NetworkError(String),
    
    /// HTTP 错误
    #[error("HTTP 错误: {0}")]
    HttpError(String),
    
    /// 超时错误
    #[error("请求超时: {0}")]
    TimeoutError(String),
    
    /// 解析错误
    #[error("解析错误: {0}")]
    ParseError(String),
    
    /// IO 错误
    #[error("IO 错误: {0}")]
    IoError(#[from] std::io::Error),
    
    /// URL 错误
    #[error("URL 错误: {0}")]
    UrlError(#[from] url::ParseError),
    
    /// 其他错误
    #[error("其他错误: {0}")]
    Other(String),
}

impl HttpError {
    /// 创建网络错误
    pub fn network(msg: impl Into<String>) -> Self {
        Self::NetworkError(msg.into())
    }
    
    /// 创建 HTTP 错误
    pub fn http(msg: impl Into<String>) -> Self {
        Self::HttpError(msg.into())
    }
    
    /// 创建超时错误
    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::TimeoutError(msg.into())
    }
    
    /// 创建解析错误
    pub fn parse(msg: impl Into<String>) -> Self {
        Self::ParseError(msg.into())
    }
    
    /// 创建其他错误
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

impl From<crate::error::RusherError> for HttpError {
    fn from(error: crate::error::RusherError) -> Self {
        match error {
            crate::error::RusherError::ConfigError(msg) => Self::Other(format!("配置错误: {}", msg)),
            crate::error::RusherError::IoError(e) => Self::IoError(e),
            crate::error::RusherError::NetworkError(msg) => Self::NetworkError(msg),
            crate::error::RusherError::DnsError(msg) => Self::NetworkError(format!("DNS错误: {}", msg)),
            crate::error::RusherError::HttpError(msg) => Self::HttpError(msg),
            crate::error::RusherError::ParseError(msg) => Self::ParseError(msg),
            crate::error::RusherError::UrlParseError(e) => Self::UrlError(e),
            crate::error::RusherError::ScanError(msg) => Self::Other(format!("扫描错误: {}", msg)),
            crate::error::RusherError::CacheError(msg) => Self::Other(format!("缓存错误: {}", msg)),
            crate::error::RusherError::SystemError(msg) => Self::Other(format!("系统错误: {}", msg)),
            crate::error::RusherError::UnknownError(msg) => Self::Other(msg),
        }
    }
}

impl From<reqwest::Error> for HttpError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            Self::TimeoutError(error.to_string())
        } else if error.is_connect() {
            Self::NetworkError(error.to_string())
        } else if error.is_status() {
            Self::HttpError(error.to_string())
        } else {
            Self::Other(error.to_string())
        }
    }
}