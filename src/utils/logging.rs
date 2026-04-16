//! 日志工具模块

use tracing_subscriber::EnvFilter;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use std::path::Path;
use crate::error::Result;

/// 初始化日志系统
pub fn init_logging(log_level: &str, log_file: Option<&str>) -> Result<()> {
    // 创建环境过滤器
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));
    
    // 如果指定了日志文件，使用文件输出
    if let Some(log_file) = log_file {
        let file_appender = RollingFileAppender::new(
            Rotation::DAILY,
            Path::new(log_file).parent().unwrap_or(Path::new(".")),
            Path::new(log_file).file_name().unwrap_or_default(),
        );
        
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_writer(file_appender)
            .with_ansi(false)
            .with_level(true)
            .with_target(true)
            .with_thread_ids(false)
            .with_thread_names(false)
            .init();
    } else {
        // 使用控制台输出
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_writer(std::io::stdout)
            .with_ansi(true)
            .with_level(true)
            .with_target(true)
            .with_thread_ids(false)
            .with_thread_names(false)
            .init();
    }
    
    tracing::info!("日志系统初始化完成，日志级别: {}", log_level);
    if let Some(log_file) = log_file {
        tracing::info!("日志文件: {}", log_file);
    }
    
    Ok(())
}

/// 日志级别工具函数
pub mod level {
    use tracing::Level;
    
    /// 将字符串转换为日志级别
    pub fn from_str(level: &str) -> Option<Level> {
        match level.to_lowercase().as_str() {
            "trace" => Some(Level::TRACE),
            "debug" => Some(Level::DEBUG),
            "info" => Some(Level::INFO),
            "warn" => Some(Level::WARN),
            "error" => Some(Level::ERROR),
            _ => None,
        }
    }
    
    /// 获取所有有效的日志级别
    pub fn all_levels() -> Vec<&'static str> {
        vec!["trace", "debug", "info", "warn", "error"]
    }
}

/// 日志宏的便捷包装
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        tracing::error!($($arg)*)
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        tracing::warn!($($arg)*)
    };
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        tracing::info!($($arg)*)
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        tracing::debug!($($arg)*)
    };
}

#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        tracing::trace!($($arg)*)
    };
}