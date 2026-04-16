//! DNS 服务器模块
//! 
//! 负责处理 DNS 查询和响应，拦截 GitHub 相关域名的 DNS 查询，
//! 返回最快的 IP 地址。

pub mod server;
pub mod cache;
pub mod resolver;

pub use server::DnsServer;
pub use cache::DnsCache;
pub use resolver::DnsResolver;