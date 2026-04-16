//! HTTP 响应定义

use std::time::Duration;

/// HTTP 响应
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// 状态码
    pub status: u16,
    /// 响应头
    pub headers: Vec<(String, String)>,
    /// 响应体
    pub body: Option<Vec<u8>>,
    /// 响应时间
    pub response_time: Duration,
    /// 内容长度
    pub content_length: Option<u64>,
    /// 内容类型
    pub content_type: Option<String>,
}

impl HttpResponse {
    /// 创建新的 HTTP 响应
    pub fn new(
        status: u16,
        headers: Vec<(String, String)>,
        body: Option<Vec<u8>>,
        response_time: Duration,
    ) -> Self {
        let content_length = body.as_ref().map(|b| b.len() as u64);
        let content_type = headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            .map(|(_, value)| value.clone());
        
        Self {
            status,
            headers,
            body,
            response_time,
            content_length,
            content_type,
        }
    }
    
    /// 检查响应是否成功
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }
    
    /// 检查响应是否为重定向
    pub fn is_redirect(&self) -> bool {
        self.status >= 300 && self.status < 400
    }
    
    /// 检查响应是否为客户端错误
    pub fn is_client_error(&self) -> bool {
        self.status >= 400 && self.status < 500
    }
    
    /// 检查响应是否为服务器错误
    pub fn is_server_error(&self) -> bool {
        self.status >= 500 && self.status < 600
    }
    
    /// 获取响应体文本
    pub fn text(&self) -> Option<String> {
        self.body.as_ref().and_then(|b| String::from_utf8(b.clone()).ok())
    }
    
    /// 获取响应时间（毫秒）
    pub fn response_time_ms(&self) -> u64 {
        self.response_time.as_millis() as u64
    }
    
    /// 获取响应时间（秒）
    pub fn response_time_secs(&self) -> f64 {
        self.response_time.as_secs_f64()
    }
    
    /// 获取响应头值
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
    
    /// 获取所有响应头值
    pub fn headers(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
            .collect()
    }
}