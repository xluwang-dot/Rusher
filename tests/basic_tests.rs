//! 测试 Rusher 项目结构

use rusher::{Config, ConfigLoader, Result};

#[tokio::test]
async fn test_config_loading() -> Result<()> {
    // 测试配置加载
    let config_loader = ConfigLoader::new();
    let config = config_loader.load()?;

    assert_eq!(config.general.log_level, "info");
    assert_eq!(config.dns.listen_addr.to_string(), "0.0.0.0:53");
    assert!(!config.github.domains.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_app_info() {
    // 测试应用程序信息
    let app_info = rusher::AppInfo::get();

    assert_eq!(app_info.name, "rusher-rs");
    assert_eq!(app_info.version, "0.1.0");
    assert_eq!(app_info.description, "GitHub DNS加速服务 - Rust实现");
    assert_eq!(app_info.authors, "Rusher Team");
}

#[test]
fn test_error_types() {
    // 测试错误类型
    use rusher::error::RusherError;

    let config_error = RusherError::ConfigError("test".to_string());
    assert!(matches!(config_error, RusherError::ConfigError(_)));

    let io_error = RusherError::IoError(std::io::Error::new(std::io::ErrorKind::Other, "test"));
    assert!(matches!(io_error, RusherError::IoError(_)));

    let network_error = RusherError::NetworkError("test".to_string());
    assert!(matches!(network_error, RusherError::NetworkError(_)));
}

#[test]
fn test_config_default() {
    // 测试默认配置
    let config = Config::default();

    assert_eq!(config.general.log_level, "info");
    assert_eq!(config.dns.cache_ttl, 300);
    assert_eq!(config.scanner.scan_interval, 300);
    assert!(!config.github.domains.is_empty());
    assert_eq!(config.http.user_agent, "Rusher-Rust/1.0");
    assert_eq!(config.cache.scan_cache_size, 10000);
    assert!(!config.monitoring.enabled);
}
