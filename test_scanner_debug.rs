use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;

use rusher::config::Config;
use rusher::scanner::github::GithubApiClient;
use rusher::scanner::scanner::IpScanner;
use rusher::scanner::cache::ScanCache;

#[tokio::main]
async fn main() {
    println!("=== 测试扫描器 ===");
    
    // 创建配置
    let config = Arc::new(Config::default());
    
    // 创建 GitHub API 客户端
    let github_client = Arc::new(GithubApiClient::new(config.clone()).unwrap());
    
    // 创建扫描缓存
    let cache = Arc::new(ScanCache::new(config.clone()));
    
    // 创建 HTTP 客户端
    let http_client = Arc::new(rusher::http::HttpClient::new(config.clone()).unwrap());
    
    // 创建扫描器
    let scanner = IpScanner::new(
        config.clone(),
        cache.clone(),
        github_client.clone(),
        http_client.clone(),
    );
    
    // 测试获取 IP 范围
    println!("获取 GitHub IP 范围...");
    match github_client.get_ip_ranges().await {
        Ok(ranges) => {
            println!("成功获取到 {} 个 IP 范围", ranges.len());
            
            // 只显示前 10 个范围
            for (i, range) in ranges.iter().take(10).enumerate() {
                println!("  {}. {}", i + 1, range);
            }
            if ranges.len() > 10 {
                println!("  ... 还有 {} 个范围", ranges.len() - 10);
            }
        }
        Err(e) => {
            println!("获取 IP 范围失败: {}", e);
        }
    }
    
    // 测试提取 IP 地址
    println!("\n测试提取 IP 地址...");
    let test_domain = "github.com";
    
    // 获取 IP 范围
    let ip_ranges = github_client.get_ip_ranges().await.unwrap();
    
    // 提取 IP 地址
    let ips = scanner.extract_ips_for_domain(test_domain, &ip_ranges);
    
    println!("为 {} 提取到 {} 个 IP 地址", test_domain, ips.len());
    
    // 只显示前 10 个 IP
    for (i, ip) in ips.iter().take(10).enumerate() {
        println!("  {}. {}", i + 1, ip);
    }
    if ips.len() > 10 {
        println!("  ... 还有 {} 个 IP", ips.len() - 10);
    }
    
    // 测试扫描少量 IP
    println!("\n测试扫描少量 IP...");
    let test_ips: Vec<IpAddr> = vec![
        IpAddr::V4(Ipv4Addr::new(140, 82, 112, 3)),
        IpAddr::V4(Ipv4Addr::new(140, 82, 113, 3)),
        IpAddr::V4(Ipv4Addr::new(140, 82, 114, 3)),
    ];
    
    let scan_results = scanner.scan_ips(test_domain, &test_ips).await;
    
    println!("扫描结果:");
    for result in scan_results {
        println!("  {}: {}ms, 可用: {}", result.ip, result.response_time, result.available);
    }
    
    println!("\n测试完成");
}