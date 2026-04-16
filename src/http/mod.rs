//! HTTP 客户端模块
//! 
//! 提供 HTTP 客户端功能，用于测试连接和测量响应时间。

mod client;
mod error;
mod request;
mod response;

pub use client::HttpClient;
pub use error::HttpError;
pub use request::HttpRequest;
pub use response::HttpResponse;

/// HTTP 客户端 trait
pub trait HttpClientTrait: Send + Sync {
    /// 测试连接到指定 IP 地址和域名
    fn test_ip_domain(&self, ip: &str, domain: &str, use_https: bool) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<std::time::Duration>> + Send + '_>>;
}

/// HTTP 模块结果类型
pub type Result<T> = std::result::Result<T, HttpError>;