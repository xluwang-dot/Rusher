//! DNS 服务器实现

use std::sync::Arc;
use tokio::net::UdpSocket;
use hickory_proto::{
    op::{Message, MessageType, OpCode, ResponseCode},
    rr::{Name, RecordType},
    serialize::binary::{BinDecodable, BinEncodable, BinEncoder},
};
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::error::{RusherError, Result};
use super::cache::DnsCache;
use super::resolver::DnsResolver;

/// DNS 服务器
pub struct DnsServer {
    /// 服务器配置
    config: Arc<Config>,
    /// DNS 缓存
    cache: Arc<DnsCache>,
    /// DNS 解析器
    resolver: Arc<DnsResolver>,
    /// UDP socket
    socket: Option<UdpSocket>,
}

impl DnsServer {
    /// 创建新的 DNS 服务器
    pub fn new(config: Arc<Config>, cache: Arc<DnsCache>, resolver: Arc<DnsResolver>) -> Self {
        Self {
            config,
            cache,
            resolver,
            socket: None,
        }
    }

    /// 启动 DNS 服务器
    pub async fn start(&mut self) -> Result<()> {
        let addr = self.config.dns.listen_addr;
        
        info!("启动 DNS 服务器，监听地址: {}", addr);
        
        // 创建 UDP socket
        let socket = UdpSocket::bind(addr).await.map_err(|e| {
            RusherError::IoError(e)
        })?;
        
        self.socket = Some(socket);
        
        // 开始处理请求
        self.run().await?;
        
        Ok(())
    }

    /// 运行 DNS 服务器
    async fn run(&mut self) -> Result<()> {
        let socket = self.socket.as_ref()
            .ok_or_else(|| RusherError::SystemError("DNS socket not initialized".to_string()))?;
        
        info!("DNS 服务器已启动，等待请求...");
        
        let mut buf = [0u8; 512];
        
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, src_addr)) => {
                    debug!("收到 DNS 请求，来源: {}，长度: {}", src_addr, len);
                    
                    // 处理请求
                    let response = self.handle_request(&buf[..len]).await;
                    
                    // 发送响应
                    if let Ok(response_data) = response {
                        if let Err(e) = socket.send_to(&response_data, src_addr).await {
                            warn!("发送 DNS 响应失败: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("接收 DNS 请求失败: {}", e);
                }
            }
        }
    }

    /// 处理 DNS 请求
    async fn handle_request(&self, request_data: &[u8]) -> Result<Vec<u8>> {
        // 解析 DNS 请求
        let request = match Message::from_bytes(request_data) {
            Ok(msg) => msg,
            Err(e) => {
                warn!("解析 DNS 请求失败: {}", e);
                return self.create_error_response(request_data, ResponseCode::FormErr);
            }
        };

        // 检查请求类型
        if request.op_code() != OpCode::Query {
            warn!("不支持的操作码: {:?}", request.op_code());
            return self.create_error_response(request_data, ResponseCode::NotImp);
        }

        // 处理查询
        let response = self.process_queries(request).await;
        
        // 序列化响应
        let mut response_data = Vec::with_capacity(512);
        let mut encoder = BinEncoder::new(&mut response_data);
        
        match response {
            Ok(response_msg) => {
                match response_msg.emit(&mut encoder) {
                    Ok(_) => Ok(response_data),
                    Err(e) => {
                        error!("序列化 DNS 响应失败: {}", e);
                        self.create_error_response(request_data, ResponseCode::ServFail)
                    }
                }
            }
            Err(e) => {
                error!("处理 DNS 查询失败: {}", e);
                self.create_error_response(request_data, ResponseCode::ServFail)
            }
        }
    }

    /// 处理 DNS 查询
    async fn process_queries(&self, request: Message) -> Result<Message> {
        let mut response = Message::new();
        
        // 设置响应头
        response.set_id(request.id());
        response.set_message_type(MessageType::Response);
        response.set_op_code(request.op_code());
        response.set_recursion_desired(request.recursion_desired());
        response.set_recursion_available(true);
        response.set_response_code(ResponseCode::NoError);
        
        // 处理每个查询
        for query in request.queries() {
            let name = query.name().clone();
            let query_type = query.query_type();
            
            debug!("处理 DNS 查询: {} {:?}", name, query_type);
            
            // 检查是否是 GitHub 域名
            let is_github_domain = self.is_github_domain(&name);
            
            if is_github_domain {
                // 处理 GitHub 域名查询
                self.handle_github_query(&mut response, &name, query_type).await?;
            } else {
                // 转发到上游 DNS
                self.handle_upstream_query(&mut response, &name, query_type).await?;
            }
        }
        
        Ok(response)
    }

    /// 检查是否是 GitHub 域名
    fn is_github_domain(&self, name: &Name) -> bool {
        let domain_str = name.to_string();
        
        for github_domain in &self.config.github.domains {
            if domain_str.ends_with(github_domain) || domain_str == *github_domain {
                return true;
            }
        }
        
        false
    }

    /// 处理 GitHub 域名查询
    async fn handle_github_query(
        &self,
        response: &mut Message,
        name: &Name,
        query_type: RecordType,
    ) -> Result<()> {
        debug!("处理 GitHub 域名查询: {} {:?}", name, query_type);
        
        // 检查缓存
        if let Some(records) = self.cache.get(name, query_type) {
            debug!("从缓存获取 DNS 记录");
            for record in records {
                response.add_answer(record.clone());
            }
            return Ok(());
        }
        
        // 从解析器获取记录
        match self.resolver.resolve(name, query_type).await {
            Ok(records) => {
                // 添加到响应
                for record in &records {
                    response.add_answer(record.clone());
                }
                
                // 更新缓存
                self.cache.set(name, query_type, records.clone());
                
                Ok(())
            }
            Err(e) => {
                warn!("解析 GitHub 域名失败: {}，错误: {}", name, e);
                Err(e)
            }
        }
    }

    /// 处理上游 DNS 查询
    async fn handle_upstream_query(
        &self,
        _response: &mut Message,
        name: &Name,
        query_type: RecordType,
    ) -> Result<()> {
        debug!("转发到上游 DNS: {} {:?}", name, query_type);
        
        // 这里应该实现上游 DNS 查询逻辑
        // 暂时返回空响应
        warn!("上游 DNS 查询功能尚未实现: {}", name);
        
        Ok(())
    }

    /// 创建错误响应
    fn create_error_response(&self, request_data: &[u8], rcode: ResponseCode) -> Result<Vec<u8>> {
        let mut response = match Message::from_bytes(request_data) {
            Ok(msg) => msg,
            Err(_) => {
                // 如果无法解析请求，创建简单的错误响应
                let mut msg = Message::new();
                msg.set_id(0);
                msg.set_message_type(MessageType::Response);
                msg.set_response_code(rcode);
                msg
            }
        };
        
        response.set_message_type(MessageType::Response);
        response.set_response_code(rcode);
        
        let mut response_data = Vec::with_capacity(512);
        let mut encoder = BinEncoder::new(&mut response_data);
        
        match response.emit(&mut encoder) {
            Ok(_) => Ok(response_data),
            Err(e) => {
                error!("创建错误响应失败: {}", e);
                Err(RusherError::DnsError(format!("创建错误响应失败: {}", e)))
            }
        }
    }

    /// 停止 DNS 服务器
    pub async fn stop(&self) -> Result<()> {
        info!("停止 DNS 服务器");
        Ok(())
    }
}