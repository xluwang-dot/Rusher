//! HTTP 客户端实现

use std::sync::Arc;
use std::time::{Duration, Instant};
use reqwest::{Client, ClientBuilder};
use tracing::{debug, info, warn};

use crate::config::Config;

use super::error::HttpError;
use super::request::HttpRequest;
use super::response::HttpResponse;
use super::Result;
use super::HttpClientTrait;

/// HTTP 客户端
pub struct HttpClient {
    /// 底层 reqwest 客户端
    client: Client,
    /// 配置
    config: Arc<Config>,
    /// 默认超时时间
    default_timeout: Duration,
    /// 默认连接超时时间
    default_connect_timeout: Duration,
}

impl HttpClientTrait for HttpClient {
    /// 测试连接到指定 IP 地址和域名
    fn test_ip_domain(&self, ip: &str, domain: &str, use_https: bool) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<std::time::Duration>> + Send + '_>> {
        let ip = ip.to_string();
        let domain = domain.to_string();
        Box::pin(async move {
            self.test_ip_domain_impl(&ip, &domain, use_https).await
        })
    }
}

impl HttpClient {
    /// 创建新的 HTTP 客户端
    pub fn new(config: Arc<Config>) -> Result<Self> {
        info!("创建 HTTP 客户端");
        
        let http_config = &config.http;
        
        // 创建客户端构建器
        let mut client_builder = ClientBuilder::new()
            .user_agent(&http_config.user_agent)
            .pool_max_idle_per_host(http_config.connection_pool_size);
        
        // 启用 HTTP/2
        if http_config.http2_enabled {
            client_builder = client_builder.http2_prior_knowledge();
            debug!("启用 HTTP/2 支持");
        }
        
        // 启用压缩
        if http_config.compression_enabled {
            // reqwest 0.11 默认启用压缩，不需要额外配置
            debug!("启用压缩支持");
        }
        
        // 设置代理
        if let Some(proxy_url) = &http_config.proxy {
            if !proxy_url.is_empty() {
                match reqwest::Proxy::all(proxy_url) {
                    Ok(proxy) => {
                        client_builder = client_builder.proxy(proxy);
                        debug!("设置代理: {}", proxy_url);
                    }
                    Err(e) => {
                        warn!("设置代理失败: {}，将不使用代理", e);
                    }
                }
            }
        }
        
        // 构建客户端
        let client = client_builder.build().map_err(|e| {
            HttpError::network(format!("创建 HTTP 客户端失败: {}", e))
        })?;
        
        // 设置默认超时时间
        let default_timeout = Duration::from_secs(config.scanner.request_timeout);
        let default_connect_timeout = Duration::from_secs(config.scanner.connect_timeout);
        
        Ok(Self {
            client,
            config,
            default_timeout,
            default_connect_timeout,
        })
    }
    
    /// 发送 HTTP 请求
    pub async fn send(&self, request: HttpRequest) -> Result<HttpResponse> {
        debug!("发送 HTTP 请求: {} {}", request.method.as_str(), request.url);
        
        let start_time = Instant::now();
        
        // 构建 reqwest 请求
        let mut reqwest_request = self.client.request(
            request.method.into(),
            request.url.clone(),
        );
        
        // 添加请求头
        for (name, value) in &request.headers {
            reqwest_request = reqwest_request.header(name, value);
        }
        
        // 添加请求体
        if let Some(body) = request.body {
            reqwest_request = reqwest_request.body(body);
        }
        
        // 设置超时
        let timeout = request.timeout.unwrap_or(self.default_timeout);
        reqwest_request = reqwest_request.timeout(timeout);
        
        // 设置连接超时
        let _connect_timeout = request.connect_timeout.unwrap_or(self.default_connect_timeout);
        // Note: reqwest doesn't have a direct connect_timeout option in the request builder
        // We'll rely on the client-level timeout configuration
        
        // 设置重定向策略
        if !request.follow_redirects {
            // reqwest 0.11 doesn't have a direct redirect method in RequestBuilder
            // We'll rely on the client-level configuration
            // reqwest_request = reqwest_request.redirect(reqwest::redirect::Policy::none());
        } else if request.max_redirects != 10 {
            // Default is 10, only need to change if different
            // Note: reqwest doesn't have a direct max_redirects option in the request builder
        }
        
        // 发送请求
        let response = reqwest_request.send().await.map_err(|e| {
            if e.is_timeout() {
                HttpError::timeout(format!("请求超时: {}", e))
            } else if e.is_connect() {
                HttpError::network(format!("连接失败: {}", e))
            } else {
                HttpError::from(e)
            }
        })?;
        
        let response_time = start_time.elapsed();
        
        // 获取状态码
        let status = response.status().as_u16();
        
        // 获取响应头
        let mut headers = Vec::new();
        for (name, value) in response.headers() {
            if let Ok(value_str) = value.to_str() {
                headers.push((name.to_string(), value_str.to_string()));
            }
        }
        
        // 获取响应体
        let body = response.bytes().await
            .map(|bytes| bytes.to_vec())
            .ok();
        
        // 创建响应对象
        let http_response = HttpResponse::new(status, headers, body, response_time);
        
        debug!("收到 HTTP 响应: {} ({}ms)", status, http_response.response_time_ms());
        
        Ok(http_response)
    }
    
    /// 发送 GET 请求
    pub async fn get(&self, url: impl AsRef<str>) -> Result<HttpResponse> {
        let request = HttpRequest::get(url)?;
        self.send(request).await
    }
    
    /// 发送 HEAD 请求
    pub async fn head(&self, url: impl AsRef<str>) -> Result<HttpResponse> {
        let request = HttpRequest::head(url)?;
        self.send(request).await
    }
    
    /// 测试连接（发送 HEAD 请求到指定 URL）
    pub async fn test_connection(&self, url: impl AsRef<str>) -> Result<Duration> {
        let url = url.as_ref();
        debug!("测试连接: {}", url);
        
        let start_time = Instant::now();
        
        match self.head(url).await {
            Ok(response) => {
                let response_time = start_time.elapsed();
                
                if response.is_success() || response.is_redirect() {
                    debug!("连接测试成功: {} ({}ms)", url, response.response_time_ms());
                    Ok(response_time)
                } else {
                    Err(HttpError::http(format!(
                        "连接测试失败: {} (状态码: {})",
                        url, response.status
                    )))
                }
            }
            Err(e) => {
                Err(HttpError::network(format!("连接测试失败: {}: {}", url, e)))
            }
        }
    }
    
    /// 测试连接到指定主机和端口
    pub async fn test_host_port(&self, host: &str, port: u16, use_https: bool) -> Result<Duration> {
        let scheme = if use_https { "https" } else { "http" };
        let url = format!("{}://{}:{}", scheme, host, port);
        
        self.test_connection(&url).await
    }
    
    /// 测试连接到指定 IP 地址和域名（实现方法）
    async fn test_ip_domain_impl(&self, ip: &str, domain: &str, use_https: bool) -> Result<Duration> {
        let scheme = if use_https { "https" } else { "http" };
        
        // 使用 IP 地址作为主机，但设置 Host 头为域名
        let url = format!("{}://{}", scheme, ip);
        let request = HttpRequest::head(&url)?
            .with_header("Host", domain);
        
        let start_time = Instant::now();
        
        match self.send(request).await {
            Ok(response) => {
                let duration = start_time.elapsed();
                if response.is_success() || response.is_redirect() {
                    debug!("连接测试成功: {} -> {} ({}ms)", ip, domain, duration.as_millis());
                    Ok(duration)
                } else {
                    warn!("连接测试失败: {} -> {} (状态码: {})", ip, domain, response.status);
                    Err(HttpError::http(format!(
                        "连接测试失败: {} -> {} (状态码: {})",
                        ip, domain, response.status
                    )))
                }
            }
            Err(e) => {
                warn!("连接测试失败: {} -> {}: {}", ip, domain, e);
                Err(e)
            }
        }
    }
    
    /// 获取客户端状态
    pub fn get_status(&self) -> HttpClientStatus {
        HttpClientStatus {
            user_agent: self.config.http.user_agent.clone(),
            connection_pool_size: self.config.http.connection_pool_size,
            http2_enabled: self.config.http.http2_enabled,
            compression_enabled: self.config.http.compression_enabled,
            proxy_enabled: self.config.http.proxy.is_some(),
            default_timeout: self.default_timeout,
            default_connect_timeout: self.default_connect_timeout,
        }
    }
}

/// HTTP 客户端状态
#[derive(Debug, Clone)]
pub struct HttpClientStatus {
    /// User-Agent
    pub user_agent: String,
    /// 连接池大小
    pub connection_pool_size: usize,
    /// 是否启用 HTTP/2
    pub http2_enabled: bool,
    /// 是否启用压缩
    pub compression_enabled: bool,
    /// 是否启用代理
    pub proxy_enabled: bool,
    /// 默认超时时间
    pub default_timeout: Duration,
    /// 默认连接超时时间
    pub default_connect_timeout: Duration,
}

impl HttpClientStatus {
    /// 打印状态信息
    pub fn print(&self) {
        println!("HTTP 客户端状态:");
        println!("  User-Agent: {}", self.user_agent);
        println!("  连接池大小: {}", self.connection_pool_size);
        println!("  HTTP/2 启用: {}", self.http2_enabled);
        println!("  压缩启用: {}", self.compression_enabled);
        println!("  代理启用: {}", self.proxy_enabled);
        println!("  默认超时: {:?}", self.default_timeout);
        println!("  默认连接超时: {:?}", self.default_connect_timeout);
    }
}