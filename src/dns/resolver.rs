//! DNS 解析器模块

use std::sync::Arc;
use hickory_proto::rr::{rdata::A, rdata::AAAA, Name, RData, Record, RecordType};
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::error::{RusherError, Result};
use crate::scanner::IpScanner;

/// DNS 解析器
pub struct DnsResolver {
    /// 配置
    config: Arc<Config>,
    /// IP 扫描器
    scanner: Arc<IpScanner>,
}

impl DnsResolver {
    /// 创建新的 DNS 解析器
    pub fn new(config: Arc<Config>, scanner: Arc<IpScanner>) -> Self {
        Self {
            config,
            scanner,
        }
    }

    /// 解析域名
    pub async fn resolve(&self, name: &Name, query_type: RecordType) -> Result<Vec<Record>> {
        debug!("解析域名: {} {:?}", name, query_type);
        
        match query_type {
            RecordType::A => self.resolve_a(name).await,
            RecordType::AAAA => self.resolve_aaaa(name).await,
            RecordType::CNAME => self.resolve_cname(name).await,
            RecordType::MX => self.resolve_mx(name).await,
            RecordType::TXT => self.resolve_txt(name).await,
            RecordType::NS => self.resolve_ns(name).await,
            RecordType::SOA => self.resolve_soa(name).await,
            _ => {
                warn!("不支持的记录类型: {:?}", query_type);
                Err(RusherError::DnsError(format!(
                    "不支持的记录类型: {:?}",
                    query_type
                )))
            }
        }
    }

    /// 解析 A 记录（IPv4）
    async fn resolve_a(&self, name: &Name) -> Result<Vec<Record>> {
        let domain = name.to_string();
        debug!("解析 A 记录: {}", domain);
        
        // 从扫描器获取最快的 IPv4 地址
        let fastest_ipv4 = self.scanner.get_fastest_ipv4(&domain).await;
        
        match fastest_ipv4 {
            Some(ip) => {
                debug!("找到最快的 IPv4 地址: {} -> {}", domain, ip);
                
                let record = Record::from_rdata(
                    name.clone(),
                    self.config.dns.cache_ttl as u32,
                    RData::A(A::from(ip)),
                );
                
                Ok(vec![record])
            }
            None => {
                warn!("未找到 {} 的 IPv4 地址", domain);
                Err(RusherError::DnsError(format!(
                    "未找到 {} 的 IPv4 地址",
                    domain
                )))
            }
        }
    }

    /// 解析 AAAA 记录（IPv6）
    async fn resolve_aaaa(&self, name: &Name) -> Result<Vec<Record>> {
        let domain = name.to_string();
        debug!("解析 AAAA 记录: {}", domain);
        
        // 检查是否启用 IPv6
        if !self.config.dns.enable_ipv6 {
            debug!("IPv6 未启用，跳过 AAAA 记录解析");
            return Ok(vec![]);
        }
        
        // 从扫描器获取最快的 IPv6 地址
        let fastest_ipv6 = self.scanner.get_fastest_ipv6(&domain).await;
        
        match fastest_ipv6 {
            Some(ip) => {
                debug!("找到最快的 IPv6 地址: {} -> {}", domain, ip);
                
                let record = Record::from_rdata(
                    name.clone(),
                    self.config.dns.cache_ttl as u32,
                    RData::AAAA(AAAA::from(ip)),
                );
                
                Ok(vec![record])
            }
            None => {
                warn!("未找到 {} 的 IPv6 地址", domain);
                Err(RusherError::DnsError(format!(
                    "未找到 {} 的 IPv6 地址",
                    domain
                )))
            }
        }
    }

    /// 解析 CNAME 记录
    async fn resolve_cname(&self, name: &Name) -> Result<Vec<Record>> {
        let domain = name.to_string();
        debug!("解析 CNAME 记录: {}", domain);
        
        // 这里应该实现 CNAME 解析逻辑
        // 暂时返回空响应
        warn!("CNAME 记录解析功能尚未实现: {}", domain);
        
        Ok(vec![])
    }

    /// 解析 MX 记录
    async fn resolve_mx(&self, name: &Name) -> Result<Vec<Record>> {
        let domain = name.to_string();
        debug!("解析 MX 记录: {}", domain);
        
        // 这里应该实现 MX 记录解析逻辑
        // 暂时返回空响应
        warn!("MX 记录解析功能尚未实现: {}", domain);
        
        Ok(vec![])
    }

    /// 解析 TXT 记录
    async fn resolve_txt(&self, name: &Name) -> Result<Vec<Record>> {
        let domain = name.to_string();
        debug!("解析 TXT 记录: {}", domain);
        
        // 这里应该实现 TXT 记录解析逻辑
        // 暂时返回空响应
        warn!("TXT 记录解析功能尚未实现: {}", domain);
        
        Ok(vec![])
    }

    /// 解析 NS 记录
    async fn resolve_ns(&self, name: &Name) -> Result<Vec<Record>> {
        let domain = name.to_string();
        debug!("解析 NS 记录: {}", domain);
        
        // 这里应该实现 NS 记录解析逻辑
        // 暂时返回空响应
        warn!("NS 记录解析功能尚未实现: {}", domain);
        
        Ok(vec![])
    }

    /// 解析 SOA 记录
    async fn resolve_soa(&self, name: &Name) -> Result<Vec<Record>> {
        let domain = name.to_string();
        debug!("解析 SOA 记录: {}", domain);
        
        // 这里应该实现 SOA 记录解析逻辑
        // 暂时返回空响应
        warn!("SOA 记录解析功能尚未实现: {}", domain);
        
        Ok(vec![])
    }

    /// 批量解析域名
    pub async fn resolve_batch(&self, names: &[Name], query_type: RecordType) -> Result<Vec<Vec<Record>>> {
        let mut results = Vec::with_capacity(names.len());
        
        for name in names {
            match self.resolve(name, query_type).await {
                Ok(records) => results.push(records),
                Err(e) => {
                    error!("解析域名失败: {}，错误: {}", name, e);
                    results.push(vec![]);
                }
            }
        }
        
        Ok(results)
    }

    /// 清除解析器缓存
    pub fn clear_cache(&self) {
        info!("清除 DNS 解析器缓存");
        // 这里应该实现缓存清除逻辑
    }
}