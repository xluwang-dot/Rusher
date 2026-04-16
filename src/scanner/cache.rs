//! 扫描缓存系统

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use moka::sync::Cache;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::config::Config;
use crate::error::Result;
use super::scanner::ScanResult;

/// 扫描缓存项
#[derive(Debug, Clone)]
struct ScanCacheItem {
    /// 扫描结果
    results: Vec<ScanResult>,
    /// 缓存时间
    cached_at: Instant,
    /// 缓存生存时间
    ttl: Duration,
}

/// 扫描缓存
pub struct ScanCache {
    /// 配置
    config: Arc<Config>,
    /// 扫描结果缓存（域名 -> 扫描结果）
    scan_cache: Cache<String, ScanCacheItem>,
    /// IP 地址列表缓存（域名 -> IP 列表）
    ip_cache: Cache<String, Vec<IpAddr>>,
    /// 统计信息
    stats: RwLock<CacheStats>,
}

impl ScanCache {
    /// 创建新的扫描缓存
    pub fn new(config: Arc<Config>) -> Self {
        let scan_cache_size = config.cache.scan_cache_size as u64;
        let cache_expiry = config.cache.cache_expiry_duration();
        
        info!(
            "初始化扫描缓存，大小: {}，过期时间: {:?}",
            scan_cache_size, cache_expiry
        );
        
        let scan_cache = Cache::builder()
            .max_capacity(scan_cache_size)
            .time_to_live(cache_expiry)
            .build();
        
        let ip_cache = Cache::builder()
            .max_capacity(scan_cache_size / 10) // IP 缓存较小
            .time_to_live(cache_expiry)
            .build();
        
        Self {
            config,
            scan_cache,
            ip_cache,
            stats: RwLock::new(CacheStats::default()),
        }
    }

    /// 获取域名的扫描结果
    pub fn get_scan_results(&self, domain: &str) -> Option<Vec<ScanResult>> {
        let cache_key = self.build_scan_cache_key(domain);
        
        match self.scan_cache.get(&cache_key) {
            Some(item) => {
                // 检查是否过期
                if item.cached_at.elapsed() < item.ttl {
                    debug!("扫描缓存命中: {}", domain);
                    self.update_stats(true);
                    Some(item.results.clone())
                } else {
                    debug!("扫描缓存过期: {}", domain);
                    self.scan_cache.invalidate(&cache_key);
                    self.update_stats(false);
                    None
                }
            }
            None => {
                debug!("扫描缓存未命中: {}", domain);
                self.update_stats(false);
                None
            }
        }
    }

    /// 设置域名的扫描结果
    pub fn set_scan_results(&self, domain: &str, results: Vec<ScanResult>) {
        let cache_key = self.build_scan_cache_key(domain);
        
        // 计算最小 TTL
        let ttl = self.calculate_scan_ttl(&results);
        
        let item = ScanCacheItem {
            results: results.clone(),
            cached_at: Instant::now(),
            ttl,
        };
        
        debug!("设置扫描缓存: {}，TTL: {:?}", domain, ttl);
        
        self.scan_cache.insert(cache_key, item);
    }

    /// 获取域名的 IP 地址列表
    pub fn get_ips_for_domain(&self, domain: &str) -> Option<Vec<IpAddr>> {
        let cache_key = self.build_ip_cache_key(domain);
        
        match self.ip_cache.get(&cache_key) {
            Some(ips) => {
                debug!("IP 缓存命中: {} ({} 个 IP)", domain, ips.len());
                Some(ips.clone())
            }
            None => {
                debug!("IP 缓存未命中: {}", domain);
                None
            }
        }
    }

    /// 设置域名的 IP 地址列表
    pub fn set_ips_for_domain(&self, domain: &str, ips: Vec<IpAddr>) {
        let cache_key = self.build_ip_cache_key(domain);
        
        debug!("设置 IP 缓存: {} ({} 个 IP)", domain, ips.len());
        
        self.ip_cache.insert(cache_key, ips);
    }

    /// 删除域名的扫描结果
    pub fn remove_scan_results(&self, domain: &str) {
        let cache_key = self.build_scan_cache_key(domain);
        
        debug!("删除扫描缓存: {}", domain);
        
        self.scan_cache.invalidate(&cache_key);
    }

    /// 删除域名的 IP 地址列表
    pub fn remove_ips_for_domain(&self, domain: &str) {
        let cache_key = self.build_ip_cache_key(domain);
        
        debug!("删除 IP 缓存: {}", domain);
        
        self.ip_cache.invalidate(&cache_key);
    }

    /// 清除所有缓存
    pub fn clear(&self) {
        info!("清除扫描缓存");
        
        self.scan_cache.invalidate_all();
        self.ip_cache.invalidate_all();
        
        let mut stats = self.stats.write();
        *stats = CacheStats::default();
    }

    /// 获取缓存统计信息
    pub fn get_stats(&self) -> CacheStats {
        self.stats.read().clone()
    }

    /// 构建扫描缓存键
    fn build_scan_cache_key(&self, domain: &str) -> String {
        format!("scan:{}", domain)
    }

    /// 构建 IP 缓存键
    fn build_ip_cache_key(&self, domain: &str) -> String {
        format!("ip:{}", domain)
    }

    /// 计算扫描结果的 TTL
    fn calculate_scan_ttl(&self, results: &[ScanResult]) -> Duration {
        let default_ttl = Duration::from_secs(self.config.cache.cache_expiry);
        
        if results.is_empty() {
            return default_ttl;
        }
        
        // 根据扫描结果的新鲜度计算 TTL
        let now = std::time::SystemTime::now();
        let mut min_age = Duration::MAX;
        
        for result in results {
            // 计算扫描时间
            let scanned_at = std::time::UNIX_EPOCH + std::time::Duration::from_millis(result.scanned_at_ms as u64);
            
            // 计算年龄
            if let Ok(age) = now.duration_since(scanned_at) {
                if age < min_age {
                    min_age = age;
                }
            }
        }
        
        // 如果扫描结果很新鲜，使用较长的 TTL
        // 如果扫描结果较旧，使用较短的 TTL
        if min_age < Duration::from_secs(60) {
            // 1分钟内的扫描结果，TTL 为 5分钟
            Duration::from_secs(300)
        } else if min_age < Duration::from_secs(300) {
            // 5分钟内的扫描结果，TTL 为 2分钟
            Duration::from_secs(120)
        } else {
            // 超过5分钟的扫描结果，TTL 为 30秒
            Duration::from_secs(30)
        }
    }

    /// 更新统计信息
    fn update_stats(&self, hit: bool) {
        let mut stats = self.stats.write();
        
        stats.total_requests += 1;
        if hit {
            stats.hits += 1;
        } else {
            stats.misses += 1;
        }
        
        // 计算命中率
        if stats.total_requests > 0 {
            stats.hit_rate = stats.hits as f64 / stats.total_requests as f64;
        }
        
        // 更新缓存大小
        stats.scan_cache_size = self.scan_cache.entry_count();
        stats.ip_cache_size = self.ip_cache.entry_count();
    }

    /// 导出缓存数据（用于持久化）
    pub fn export_data(&self) -> CacheExport {
        // 这里应该实现缓存数据的序列化导出
        // 暂时返回空数据
        CacheExport {
            scan_data: HashMap::new(),
            ip_data: HashMap::new(),
            stats: self.get_stats(),
        }
    }

    /// 导入缓存数据（从持久化存储加载）
    pub fn import_data(&self, _data: CacheExport) -> Result<()> {
        // 这里应该实现缓存数据的反序列化导入
        // 暂时不实现
        Ok(())
    }
}

/// 缓存统计信息
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheStats {
    /// 总请求数
    pub total_requests: u64,
    /// 命中数
    pub hits: u64,
    /// 未命中数
    pub misses: u64,
    /// 命中率
    pub hit_rate: f64,
    /// 扫描缓存大小
    pub scan_cache_size: u64,
    /// IP 缓存大小
    pub ip_cache_size: u64,
}

impl CacheStats {
    /// 打印统计信息
    pub fn print(&self) {
        println!("扫描缓存统计:");
        println!("  总请求数: {}", self.total_requests);
        println!("  命中数: {}", self.hits);
        println!("  未命中数: {}", self.misses);
        println!("  命中率: {:.2}%", self.hit_rate * 100.0);
        println!("  扫描缓存大小: {}", self.scan_cache_size);
        println!("  IP 缓存大小: {}", self.ip_cache_size);
    }
}

/// 缓存导出数据
#[derive(Debug, Clone)]
pub struct CacheExport {
    /// 扫描数据
    pub scan_data: HashMap<String, Vec<ScanResult>>,
    /// IP 数据
    pub ip_data: HashMap<String, Vec<IpAddr>>,
    /// 统计信息
    pub stats: CacheStats,
}