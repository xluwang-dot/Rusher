//! DNS 缓存模块

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use hickory_proto::rr::{Name, Record, RecordType};
use moka::sync::Cache;
use parking_lot::RwLock;
use tracing::{debug, info};

use crate::config::Config;

/// DNS 缓存项
#[derive(Debug, Clone)]
struct CacheItem {
    /// DNS 记录
    records: Vec<Record>,
    /// 缓存时间
    cached_at: Instant,
    /// 缓存生存时间
    ttl: Duration,
}

/// DNS 缓存
pub struct DnsCache {
    /// 配置
    config: Arc<Config>,
    /// 主缓存
    cache: Cache<String, CacheItem>,
    /// 反向索引（域名 -> 缓存键）
    reverse_index: RwLock<HashMap<String, Vec<String>>>,
    /// 统计信息
    stats: RwLock<CacheStats>,
}

impl DnsCache {
    /// 创建新的 DNS 缓存
    pub fn new(config: Arc<Config>) -> Self {
        let cache_size = config.cache.dns_cache_size as u64;
        let cache_expiry = config.cache_expiry_duration();
        
        info!(
            "初始化 DNS 缓存，大小: {}，过期时间: {:?}",
            cache_size, cache_expiry
        );
        
        let cache = Cache::builder()
            .max_capacity(cache_size)
            .time_to_live(cache_expiry)
            .build();
        
        Self {
            config,
            cache,
            reverse_index: RwLock::new(HashMap::new()),
            stats: RwLock::new(CacheStats::default()),
        }
    }

    /// 获取 DNS 记录
    pub fn get(&self, name: &Name, query_type: RecordType) -> Option<Vec<Record>> {
        let cache_key = self.build_cache_key(name, query_type);
        
        match self.cache.get(&cache_key) {
            Some(item) => {
                // 检查是否过期
                if item.cached_at.elapsed() < item.ttl {
                    debug!("缓存命中: {}", cache_key);
                    self.update_stats(true);
                    Some(item.records.clone())
                } else {
                    debug!("缓存过期: {}", cache_key);
                    self.cache.invalidate(&cache_key);
                    self.remove_from_index(&cache_key);
                    self.update_stats(false);
                    None
                }
            }
            None => {
                debug!("缓存未命中: {}", cache_key);
                self.update_stats(false);
                None
            }
        }
    }

    /// 设置 DNS 记录
    pub fn set(&self, name: &Name, query_type: RecordType, records: Vec<Record>) {
        let cache_key = self.build_cache_key(name, query_type);
        
        // 计算最小 TTL
        let ttl = self.calculate_min_ttl(&records);
        
        let item = CacheItem {
            records: records.clone(),
            cached_at: Instant::now(),
            ttl,
        };
        
        debug!("设置缓存: {}，TTL: {:?}", cache_key, ttl);
        
        self.cache.insert(cache_key.clone(), item);
        self.add_to_index(name, &cache_key);
    }

    /// 删除 DNS 记录
    pub fn remove(&self, name: &Name, query_type: RecordType) {
        let cache_key = self.build_cache_key(name, query_type);
        
        debug!("删除缓存: {}", cache_key);
        
        self.cache.invalidate(&cache_key);
        self.remove_from_index(&cache_key);
    }

    /// 清除所有缓存
    pub fn clear(&self) {
        info!("清除 DNS 缓存");
        
        self.cache.invalidate_all();
        self.reverse_index.write().clear();
    }

    /// 获取缓存统计信息
    pub fn stats(&self) -> CacheStats {
        self.stats.read().clone()
    }

    /// 构建缓存键
    fn build_cache_key(&self, name: &Name, query_type: RecordType) -> String {
        format!("{}:{}", name, query_type)
    }

    /// 计算最小 TTL
    fn calculate_min_ttl(&self, records: &[Record]) -> Duration {
        let default_ttl = Duration::from_secs(self.config.dns.cache_ttl as u64);
        
        if records.is_empty() {
            return default_ttl;
        }
        
        let min_ttl = records
            .iter()
            .map(|record| Duration::from_secs(record.ttl() as u64))
            .min()
            .unwrap_or(default_ttl);
        
        // 确保 TTL 不超过配置的最大值
        min_ttl.min(default_ttl)
    }

    /// 添加到反向索引
    fn add_to_index(&self, name: &Name, cache_key: &str) {
        let domain = name.to_string();
        let mut index = self.reverse_index.write();
        
        let entries = index.entry(domain).or_insert_with(Vec::new);
        if !entries.contains(&cache_key.to_string()) {
            entries.push(cache_key.to_string());
        }
    }

    /// 从反向索引中移除
    fn remove_from_index(&self, cache_key: &str) {
        let mut index = self.reverse_index.write();
        
        // 找到包含此缓存键的域名
        let mut domains_to_remove = Vec::new();
        
        for (domain, keys) in index.iter_mut() {
            keys.retain(|key| key != cache_key);
            if keys.is_empty() {
                domains_to_remove.push(domain.clone());
            }
        }
        
        // 移除空域名的条目
        for domain in domains_to_remove {
            index.remove(&domain);
        }
    }

    /// 更新统计信息
    fn update_stats(&self, hit: bool) {
        let mut stats = self.stats.write();
        
        if hit {
            stats.hits += 1;
        } else {
            stats.misses += 1;
        }
        
        let total = stats.hits + stats.misses;
        if total > 0 {
            stats.hit_rate = stats.hits as f64 / total as f64;
        }
        
        stats.entry_count = self.cache.entry_count() as u64;
        stats.size = self.cache.weighted_size();
    }
}

/// 缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// 缓存条目数量
    pub entry_count: u64,
    /// 命中数
    pub hits: u64,
    /// 未命中数
    pub misses: u64,
    /// 命中率
    pub hit_rate: f64,
    /// 缓存大小
    pub size: u64,
}

impl CacheStats {
    /// 打印统计信息
    pub fn print(&self) {
        println!("DNS 缓存统计:");
        println!("  条目数量: {}", self.entry_count);
        println!("  命中率: {:.2}%", self.hit_rate * 100.0);
        println!("  大小: {}", self.size);
    }
}