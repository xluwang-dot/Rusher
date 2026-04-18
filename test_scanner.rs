use std::sync::Arc;
use rusher::config::Config;
use rusher::scanner::scanner::IpScanner;

#[tokio::main]
async fn main() {
    // 创建配置
    let config = Config::default();
    let config_arc = Arc::new(config);
    
    // 创建扫描器
    let scanner = IpScanner::new(config_arc.clone()).unwrap();
    
    // 启动扫描器
    scanner.start().await.unwrap();
    
    // 等待一段时间让扫描器完成初始扫描
    println!("等待扫描器完成初始扫描...");
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    
    // 打印最优 IP 地址
    println!("\n=== 扫描结果 ===");
    scanner.print_optimal_ips().await;
    
    // 打印扫描统计信息
    let stats = scanner.get_stats().await;
    stats.print();
    
    // 停止扫描器
    scanner.stop().await;
}