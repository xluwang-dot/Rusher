//! 工具模块
//! 
//! 包含各种工具函数和辅助功能

pub mod logging;
pub mod signal;

// 重新导出日志宏（这些宏已经在 crate 根级别导出）
// 注意：这些宏使用 crate::log_* 语法调用，因为它们是用 #[macro_export] 导出的

/// 通用工具函数
pub mod utils {
    use std::time::{Duration, Instant};
    
    /// 格式化持续时间
    pub fn format_duration(duration: Duration) -> String {
        let secs = duration.as_secs();
        if secs < 60 {
            format!("{}秒", secs)
        } else if secs < 3600 {
            format!("{}分{}秒", secs / 60, secs % 60)
        } else {
            format!("{}时{}分{}秒", secs / 3600, (secs % 3600) / 60, secs % 60)
        }
    }
    
    /// 测量代码执行时间
    pub fn measure_time<F, R>(f: F) -> (R, Duration)
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();
        (result, duration)
    }
    
    /// 异步测量代码执行时间
    pub async fn measure_time_async<F, Fut, R>(f: F) -> (R, Duration)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = R>,
    {
        let start = Instant::now();
        let result = f().await;
        let duration = start.elapsed();
        (result, duration)
    }
    
    /// 检查字符串是否为空或只包含空白字符
    pub fn is_blank(s: &str) -> bool {
        s.trim().is_empty()
    }
    
    /// 将字节大小格式化为人类可读的字符串
    pub fn format_bytes(bytes: u64) -> String {
        const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
        
        let mut size = bytes as f64;
        let mut unit_index = 0;
        
        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }
        
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

/// 网络工具函数
pub mod network {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    
    /// 检查 IP 地址是否为私有地址
    pub fn is_private_ip(ip: &IpAddr) -> bool {
        match ip {
            IpAddr::V4(ipv4) => is_private_ipv4(ipv4),
            IpAddr::V6(ipv6) => is_private_ipv6(ipv6),
        }
    }
    
    /// 检查 IPv4 地址是否为私有地址
    pub fn is_private_ipv4(ip: &Ipv4Addr) -> bool {
        let octets = ip.octets();
        
        // 10.0.0.0/8
        if octets[0] == 10 {
            return true;
        }
        
        // 172.16.0.0/12
        if octets[0] == 172 && (16..=31).contains(&octets[1]) {
            return true;
        }
        
        // 192.168.0.0/16
        if octets[0] == 192 && octets[1] == 168 {
            return true;
        }
        
        // 169.254.0.0/16 (链路本地)
        if octets[0] == 169 && octets[1] == 254 {
            return true;
        }
        
        false
    }
    
    /// 检查 IPv6 地址是否为私有地址
    pub fn is_private_ipv6(ip: &Ipv6Addr) -> bool {
        let segments = ip.segments();
        
        // fc00::/7 (唯一本地地址)
        if segments[0] & 0xfe00 == 0xfc00 {
            return true;
        }
        
        // fe80::/10 (链路本地地址)
        if segments[0] & 0xffc0 == 0xfe80 {
            return true;
        }
        
        false
    }
    
    /// 检查 IP 地址是否为环回地址
    pub fn is_loopback_ip(ip: &IpAddr) -> bool {
        match ip {
            IpAddr::V4(ipv4) => ipv4.is_loopback(),
            IpAddr::V6(ipv6) => ipv6.is_loopback(),
        }
    }
    
    /// 检查 IP 地址是否为多播地址
    pub fn is_multicast_ip(ip: &IpAddr) -> bool {
        match ip {
            IpAddr::V4(ipv4) => ipv4.is_multicast(),
            IpAddr::V6(ipv6) => ipv6.is_multicast(),
        }
    }
}