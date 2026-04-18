//! GitHub API 客户端模块

use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::error::{RusherError, Result};

/// GitHub Meta API 响应结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubMetaResponse {
    /// GitHub 的 IP 地址范围
    #[serde(default)]
    pub hooks: Vec<String>,
    #[serde(default)]
    pub web: Vec<String>,
    #[serde(default)]
    pub api: Vec<String>,
    #[serde(default)]
    pub git: Vec<String>,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub pages: Vec<String>,
    #[serde(default)]
    pub importer: Vec<String>,
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub dependabot: Vec<String>,
}

/// GitHub API 客户端
pub struct GithubApiClient {
    /// 配置
    config: Arc<Config>,
    /// HTTP 客户端
    client: reqwest::Client,
    /// API 响应缓存
    cache: RwLock<Option<(GithubMetaResponse, Instant)>>,
    /// 缓存过期时间（秒）
    cache_ttl: Duration,
}

impl GithubApiClient {
    /// 创建新的 GitHub API 客户端
    pub fn new(config: Arc<Config>) -> Result<Self> {
        info!("创建 GitHub API 客户端");
        
        // 创建 HTTP 客户端
        let client_builder = reqwest::Client::builder()
            .user_agent(&config.http.user_agent)
            .timeout(Duration::from_secs(config.scanner.request_timeout))
            .pool_max_idle_per_host(config.http.connection_pool_size)
            .http2_prior_knowledge();
        
        // 启用 HTTP/2
        let client_builder = if config.http.http2_enabled {
            client_builder.http2_prior_knowledge()
        } else {
            client_builder
        };
        
        // 启用压缩
        let client_builder = if config.http.compression_enabled {
            // reqwest 0.11 默认启用压缩，不需要额外配置
            client_builder
        } else {
            client_builder
        };
        
        // 设置代理
        let client_builder = if let Some(proxy_url) = &config.http.proxy {
            if !proxy_url.is_empty() {
                match reqwest::Proxy::all(proxy_url) {
                    Ok(proxy) => client_builder.proxy(proxy),
                    Err(e) => {
                        warn!("设置代理失败: {}，将不使用代理", e);
                        client_builder
                    }
                }
            } else {
                client_builder
            }
        } else {
            client_builder
        };
        
        // 构建客户端
        let client = client_builder.build().map_err(|e| {
            RusherError::HttpError(format!("创建 HTTP 客户端失败: {}", e))
        })?;
        
        // 设置缓存过期时间
        let cache_ttl = Duration::from_secs(config.cache.cache_expiry);
        
        Ok(Self {
            config,
            client,
            cache: RwLock::new(None),
            cache_ttl,
        })
    }

    /// 获取 GitHub IP 范围（结构化）
    pub async fn get_ip_ranges_structured(&self) -> Result<GithubMetaResponse> {
        info!("获取 GitHub IP 范围（结构化）");
        
        // 首先检查缓存
        if let Some((response, cached_at)) = self.get_cached_response().await {
            if cached_at.elapsed() < self.cache_ttl {
                debug!("从缓存获取 IP 范围（结构化）");
                return Ok(response.clone());
            }
        }
        
        // 从 API 获取
        let response = self.fetch_ip_ranges_from_api().await?;
        
        // 更新缓存
        self.update_cache(response.clone()).await;
        
        info!("获取到结构化 IP 范围");
        
        Ok(response)
    }
    
    /// 获取 GitHub IP 范围（扁平化，向后兼容）
    pub async fn get_ip_ranges(&self) -> Result<Vec<String>> {
        info!("获取 GitHub IP 范围（扁平化）");
        
        // 首先检查缓存
        if let Some(ranges) = self.get_cached_ip_ranges().await {
            debug!("从缓存获取 IP 范围");
            return Ok(ranges);
        }
        
        // 从 API 获取
        let response = self.fetch_ip_ranges_from_api().await?;
        
        // 提取所有 IP 范围
        let mut all_ranges = Vec::new();
        
        // 添加各个类别的 IP 范围
        all_ranges.extend(response.hooks.clone());
        all_ranges.extend(response.web.clone());
        all_ranges.extend(response.api.clone());
        all_ranges.extend(response.git.clone());
        all_ranges.extend(response.packages.clone());
        all_ranges.extend(response.pages.clone());
        all_ranges.extend(response.importer.clone());
        all_ranges.extend(response.actions.clone());
        all_ranges.extend(response.dependabot.clone());
        
        // 添加自定义 IP 范围
        all_ranges.extend(self.config.github.custom_ranges.clone());
        
        // 去重
        all_ranges.sort();
        all_ranges.dedup();
        
        info!("获取到 {} 个 IP 范围", all_ranges.len());
        
        // 更新缓存
        self.update_cache(response).await;
        
        Ok(all_ranges)
    }
    
    /// 从缓存获取结构化响应
    async fn get_cached_response(&self) -> Option<(GithubMetaResponse, Instant)> {
        let cache_guard = self.cache.read().await;
        cache_guard.clone()
    }

    /// 从缓存获取 IP 范围
    async fn get_cached_ip_ranges(&self) -> Option<Vec<String>> {
        let cache_guard = self.cache.read().await;
        
        if let Some((response, cached_at)) = cache_guard.as_ref() {
            // 检查缓存是否过期
            if cached_at.elapsed() < self.cache_ttl {
                // 提取所有 IP 范围
                let mut all_ranges = Vec::new();
                
                all_ranges.extend(response.hooks.clone());
                all_ranges.extend(response.web.clone());
                all_ranges.extend(response.api.clone());
                all_ranges.extend(response.git.clone());
                all_ranges.extend(response.packages.clone());
                all_ranges.extend(response.pages.clone());
                all_ranges.extend(response.importer.clone());
                all_ranges.extend(response.actions.clone());
                all_ranges.extend(response.dependabot.clone());
                
                // 添加自定义 IP 范围
                all_ranges.extend(self.config.github.custom_ranges.clone());
                
                // 去重
                all_ranges.sort();
                all_ranges.dedup();
                
                return Some(all_ranges);
            }
        }
        
        None
    }

    /// 从 API 获取 IP 范围
    async fn fetch_ip_ranges_from_api(&self) -> Result<GithubMetaResponse> {
        let url = &self.config.github.meta_url;
        info!("从 GitHub API 获取 IP 范围: {}", url);
        
        // 构建请求
        let mut request = self.client.get(url);
        
        // 添加认证头（如果需要）
        if self.config.github.api_auth_enabled {
            if let Some(token) = &self.config.github.api_token {
                if !token.is_empty() {
                    request = request.header("Authorization", format!("Bearer {}", token));
                    debug!("使用 GitHub API Token 认证");
                }
            }
        }
        
        // 发送请求
        let response = request.send().await.map_err(|e| {
            RusherError::NetworkError(format!("请求 GitHub API 失败: {}", e))
        })?;
        
        // 检查响应状态
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            
            return Err(RusherError::HttpError(format!(
                "GitHub API 返回错误: {} - {}",
                status, body
            )));
        }
        
        // 解析响应
        let meta_response = response.json::<GithubMetaResponse>().await.map_err(|e| {
            RusherError::ParseError(format!("解析 GitHub API 响应失败: {}", e))
        })?;
        
        debug!("成功获取 GitHub IP 范围");
        
        Ok(meta_response)
    }

    /// 更新缓存
    async fn update_cache(&self, response: GithubMetaResponse) {
        let mut cache_guard = self.cache.write().await;
        *cache_guard = Some((response, Instant::now()));
        
        debug!("更新 GitHub API 缓存");
    }

    /// 清除缓存
    pub async fn clear_cache(&self) {
        let mut cache_guard = self.cache.write().await;
        *cache_guard = None;
        
        info!("清除 GitHub API 缓存");
    }

    /// 测试 API 连接
    pub async fn test_connection(&self) -> Result<()> {
        info!("测试 GitHub API 连接");
        
        let url = &self.config.github.meta_url;
        debug!("测试连接: {}", url);
        
        // 发送 HEAD 请求测试连接
        let response = self.client.head(url).send().await.map_err(|e| {
            RusherError::NetworkError(format!("测试 GitHub API 连接失败: {}", e))
        })?;
        
        if response.status().is_success() {
            info!("GitHub API 连接测试成功");
            Ok(())
        } else {
            let status = response.status();
            Err(RusherError::HttpError(format!(
                "GitHub API 连接测试失败: {}",
                status
            )))
        }
    }

    /// 获取 API 状态
    pub async fn get_api_status(&self) -> ApiStatus {
        let cache_guard = self.cache.read().await;
        
        let has_cache = cache_guard.is_some();
        let cache_valid = if let Some((_, cached_at)) = cache_guard.as_ref() {
            cached_at.elapsed() < self.cache_ttl
        } else {
            false
        };
        
        ApiStatus {
            has_cache,
            cache_valid,
            cache_ttl: self.cache_ttl,
            api_url: self.config.github.meta_url.clone(),
            auth_enabled: self.config.github.api_auth_enabled,
        }
    }
}

/// API 状态信息
#[derive(Debug, Clone)]
pub struct ApiStatus {
    /// 是否有缓存
    pub has_cache: bool,
    /// 缓存是否有效
    pub cache_valid: bool,
    /// 缓存过期时间
    pub cache_ttl: Duration,
    /// API URL
    pub api_url: String,
    /// 是否启用认证
    pub auth_enabled: bool,
}

impl ApiStatus {
    /// 打印状态信息
    pub fn print(&self) {
        println!("GitHub API 状态:");
        println!("  API URL: {}", self.api_url);
        println!("  认证启用: {}", self.auth_enabled);
        println!("  有缓存: {}", self.has_cache);
        println!("  缓存有效: {}", self.cache_valid);
        println!("  缓存过期时间: {:?}", self.cache_ttl);
    }
}