//! HTTP 请求定义

use std::time::Duration;
use url::Url;

/// HTTP 请求方法
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    PATCH,
    OPTIONS,
}

impl HttpMethod {
    /// 转换为字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GET => "GET",
            Self::HEAD => "HEAD",
            Self::POST => "POST",
            Self::PUT => "PUT",
            Self::DELETE => "DELETE",
            Self::PATCH => "PATCH",
            Self::OPTIONS => "OPTIONS",
        }
    }
}

impl From<HttpMethod> for reqwest::Method {
    fn from(method: HttpMethod) -> Self {
        match method {
            HttpMethod::GET => reqwest::Method::GET,
            HttpMethod::HEAD => reqwest::Method::HEAD,
            HttpMethod::POST => reqwest::Method::POST,
            HttpMethod::PUT => reqwest::Method::PUT,
            HttpMethod::DELETE => reqwest::Method::DELETE,
            HttpMethod::PATCH => reqwest::Method::PATCH,
            HttpMethod::OPTIONS => reqwest::Method::OPTIONS,
        }
    }
}

/// HTTP 请求配置
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// 请求方法
    pub method: HttpMethod,
    /// 请求 URL
    pub url: Url,
    /// 请求头
    pub headers: Vec<(String, String)>,
    /// 请求体
    pub body: Option<Vec<u8>>,
    /// 超时时间
    pub timeout: Option<Duration>,
    /// 连接超时时间
    pub connect_timeout: Option<Duration>,
    /// 是否跟随重定向
    pub follow_redirects: bool,
    /// 最大重定向次数
    pub max_redirects: usize,
    /// DNS 解析覆盖（将域名映射到指定 IP）
    pub resolve: Option<String>,
}

impl HttpRequest {
    /// 创建新的 GET 请求
    pub fn get(url: impl AsRef<str>) -> crate::Result<Self> {
        let url = Url::parse(url.as_ref())?;
        
        Ok(Self {
            method: HttpMethod::GET,
            url,
            headers: Vec::new(),
            body: None,
            timeout: None,
            connect_timeout: None,
            follow_redirects: true,
            max_redirects: 10,
            resolve: None,
        })
    }
    
    /// 创建新的 HEAD 请求
    pub fn head(url: impl AsRef<str>) -> crate::Result<Self> {
        let url = Url::parse(url.as_ref())?;
        
        Ok(Self {
            method: HttpMethod::HEAD,
            url,
            headers: Vec::new(),
            body: None,
            timeout: None,
            connect_timeout: None,
            follow_redirects: true,
            max_redirects: 10,
            resolve: None,
        })
    }
    
    /// 设置请求头
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
    
    /// 设置超时时间
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
    
    /// 设置连接超时时间
    pub fn with_connect_timeout(mut self, connect_timeout: Duration) -> Self {
        self.connect_timeout = Some(connect_timeout);
        self
    }
    
    /// 设置是否跟随重定向
    pub fn with_follow_redirects(mut self, follow: bool) -> Self {
        self.follow_redirects = follow;
        self
    }
    
    /// 设置最大重定向次数
    pub fn with_max_redirects(mut self, max: usize) -> Self {
        self.max_redirects = max;
        self
    }
    
    /// 设置请求体
    pub fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = Some(body);
        self
    }
    
    /// 设置 DNS 解析覆盖
    pub fn with_resolve(mut self, resolve: impl Into<String>) -> Self {
        self.resolve = Some(resolve.into());
        self
    }
    
    /// 获取主机名
    pub fn host(&self) -> Option<&str> {
        self.url.host_str()
    }
    
    /// 获取端口
    pub fn port(&self) -> Option<u16> {
        self.url.port()
    }
    
    /// 获取路径
    pub fn path(&self) -> &str {
        self.url.path()
    }
    
    /// 获取查询字符串
    pub fn query(&self) -> Option<&str> {
        self.url.query()
    }
}