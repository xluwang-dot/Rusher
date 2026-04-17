//! DNS 服务器测试程序

use rusher::{ConfigLoader, Result, HttpClient, GithubApiClient, IpScanner, DnsServer, DnsCache, DnsResolver};
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

/// 测试 DNS 服务器
#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Rusher DNS 服务器测试 ===");
    
    // 加载测试配置
    println!("加载测试配置...");
    let config = ConfigLoader::load_from_path("config/test.toml")?;
    
    // 验证配置
    rusher::config::loader::utils::validate_config(&config)?;
    
    // 打印配置摘要
    rusher::config::loader::utils::print_config_summary(&config);
    
    println!("\n启动 DNS 服务器测试...");
    
    // 创建共享配置
    let config_arc = Arc::new(config);
    
    // 创建 HTTP 客户端
    println!("创建 HTTP 客户端...");
    let http_client = Arc::new(HttpClient::new(config_arc.clone())?);
    
    // 创建 GitHub API 客户端
    println!("创建 GitHub API 客户端...");
    let github_client = Arc::new(GithubApiClient::new(config_arc.clone())?);
    
    // 创建扫描缓存
    println!("创建扫描缓存...");
    let scan_cache = Arc::new(rusher::scanner::cache::ScanCache::new(config_arc.clone()));
    
    // 创建 IP 扫描器
    println!("创建 IP 扫描器...");
    let scanner = Arc::new(IpScanner::new(
        config_arc.clone(),
        scan_cache,
        github_client,
        http_client,
    ));
    
    // 启动扫描器
    println!("启动扫描器...");
    scanner.start().await?;
    
    // 创建 DNS 缓存
    println!("创建 DNS 缓存...");
    let dns_cache = Arc::new(DnsCache::new(config_arc.clone()));
    
    // 创建 DNS 解析器
    println!("创建 DNS 解析器...");
    let dns_resolver = Arc::new(DnsResolver::new(
        config_arc.clone(),
        scanner.clone(),
    ));
    
    // 创建 DNS 服务器
    println!("创建 DNS 服务器...");
    let mut dns_server = DnsServer::new(
        config_arc.clone(),
        dns_cache,
        dns_resolver,
    );
    
    println!("DNS 服务器将在 {} 启动", config_arc.dns.listen_addr);
    println!("按 Ctrl+C 停止测试");
    
    // 在后台启动 DNS 服务器
    let dns_server_handle = tokio::spawn(async move {
        println!("正在启动 DNS 服务器...");
        match dns_server.start().await {
            Ok(_) => println!("DNS 服务器正常停止"),
            Err(e) => eprintln!("DNS 服务器启动失败: {}", e),
        }
    });
    
    // 等待一段时间让服务器启动
    println!("等待服务器启动...");
    time::sleep(Duration::from_secs(2)).await;
    
    // 测试 DNS 查询
    println!("\n测试 DNS 查询...");
    test_dns_query(&config_arc.dns.listen_addr.to_string()).await?;
    
    // 等待用户按 Ctrl+C
    println!("\n等待 10 秒后自动停止测试...");
    time::sleep(Duration::from_secs(10)).await;
    
    println!("停止测试...");
    
    // 停止扫描器
    scanner.stop().await?;
    
    // 停止 DNS 服务器
    dns_server_handle.abort();
    
    println!("测试完成!");
    Ok(())
}

/// 测试 DNS 查询
async fn test_dns_query(listen_addr: &str) -> Result<()> {
    use hickory_proto::op::{Message, Query};
    use hickory_proto::rr::{Name, RecordType};
    use hickory_proto::serialize::binary::{BinEncodable, BinDecodable};
    use std::net::UdpSocket;
    use std::time::Duration;
    
    // 解析监听地址
    let socket_addr: std::net::SocketAddr = listen_addr.parse().map_err(|e| {
        rusher::error::RusherError::ParseError(format!("解析地址失败: {}: {}", listen_addr, e))
    })?;
    
    // 创建 UDP socket
    let socket = UdpSocket::bind("127.0.0.1:0").map_err(|e| {
        rusher::error::RusherError::IoError(e)
    })?;
    
    socket.set_read_timeout(Some(Duration::from_secs(2))).map_err(|e| {
        rusher::error::RusherError::IoError(e)
    })?;
    
    // 创建 DNS 查询
    let mut message = Message::new();
    message.set_id(1234);
    
    let name = Name::from_utf8("github.com").map_err(|e| {
        rusher::error::RusherError::ParseError(format!("创建域名失败: {}", e))
    })?;
    
    let query = Query::query(name, RecordType::A);
    message.add_query(query);
    
    // 序列化消息
    let mut buf = Vec::new();
    let mut encoder = hickory_proto::serialize::binary::BinEncoder::new(&mut buf);
    message.emit(&mut encoder).map_err(|e| {
        rusher::error::RusherError::DnsError(format!("序列化 DNS 消息失败: {}", e))
    })?;
    
    // 发送查询
    println!("发送 DNS 查询到 {}...", socket_addr);
    socket.send_to(&buf, socket_addr).map_err(|e| {
        rusher::error::RusherError::IoError(e)
    })?;
    
    // 接收响应
    let mut response_buf = [0u8; 512];
    match socket.recv_from(&mut response_buf) {
        Ok((len, src_addr)) => {
            println!("收到 DNS 响应，来源: {}，长度: {}", src_addr, len);
            
            // 解析响应
            let response = Message::from_bytes(&response_buf[..len]).map_err(|e| {
                rusher::error::RusherError::ParseError(format!("解析 DNS 响应失败: {}", e))
            })?;
            
            println!("DNS 响应 ID: {}", response.id());
            println!("响应码: {:?}", response.response_code());
            println!("回答数量: {}", response.answer_count());
            
            if response.answer_count() > 0 {
                for answer in response.answers() {
                    println!("回答: {}", answer);
                }
            } else {
                println!("没有回答记录");
            }
            
            Ok(())
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut {
                println!("DNS 查询超时，服务器可能未响应");
                Ok(())
            } else {
                Err(rusher::error::RusherError::IoError(e))
            }
        }
    }
}