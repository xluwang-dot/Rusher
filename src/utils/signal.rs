//! 信号处理模块
//! 
//! 处理系统信号，如 Ctrl+C、SIGTERM 等

use tokio::signal;
use crate::error::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

/// 信号处理器
pub struct SignalHandler {
    shutdown: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl SignalHandler {
    /// 创建新的信号处理器
    pub fn new() -> Self {
        Self {
            shutdown: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
        }
    }
    
    /// 获取关机标志的引用
    pub fn shutdown_flag(&self) -> Arc<AtomicBool> {
        self.shutdown.clone()
    }
    
    /// 获取通知器的引用
    pub fn notify(&self) -> Arc<Notify> {
        self.notify.clone()
    }
    
    /// 启动信号监听
    pub async fn listen(&self) -> Result<()> {
        let shutdown = self.shutdown.clone();
        let notify = self.notify.clone();
        
        tokio::spawn(async move {
            #[cfg(unix)]
            let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("无法注册 SIGTERM 处理器");
            #[cfg(unix)]
            let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())
                .expect("无法注册 SIGINT 处理器");
            
            #[cfg(unix)]
            {
                let ctrl_c = signal::ctrl_c();
                tokio::select! {
                    _ = ctrl_c => {
                        crate::log_info!("收到 Ctrl+C 信号，开始优雅关机");
                    }
                    _ = sigterm.recv() => {
                        crate::log_info!("收到 SIGTERM 信号，开始优雅关机");
                    }
                    _ = sigint.recv() => {
                        crate::log_info!("收到 SIGINT 信号，开始优雅关机");
                    }
                }
            }
            
            #[cfg(not(unix))]
            {
                let ctrl_c = signal::ctrl_c();
                ctrl_c.await.expect("等待 Ctrl+C 信号失败");
                crate::log_info!("收到 Ctrl+C 信号，开始优雅关机");
            }
            
            // 设置关机标志
            shutdown.store(true, Ordering::SeqCst);
            
            // 通知等待的线程
            notify.notify_waiters();
            
            crate::log_info!("信号处理完成，等待服务关闭");
        });
        
        Ok(())
    }
    
    /// 检查是否收到关机信号
    pub fn should_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }
    
    /// 等待关机信号
    pub async fn wait_for_shutdown(&self) {
        if !self.should_shutdown() {
            self.notify.notified().await;
        }
    }
    
    /// 等待指定时间或直到收到关机信号
    pub async fn wait_for_shutdown_or_timeout(&self, duration: std::time::Duration) -> bool {
        tokio::select! {
            _ = self.wait_for_shutdown() => {
                true
            }
            _ = tokio::time::sleep(duration) => {
                false
            }
        }
    }
}

impl Default for SignalHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// 信号处理工具函数
pub mod utils {
    use super::*;
    
    /// 创建并启动信号处理器
    pub async fn create_and_start_signal_handler() -> Result<SignalHandler> {
        let handler = SignalHandler::new();
        handler.listen().await?;
        Ok(handler)
    }
    
    /// 检查是否应该关机
    pub fn should_shutdown() -> bool {
        // 这里可以使用全局状态，但为了简单起见，我们返回 false
        // 在实际应用中，可以使用 once_cell 或 lazy_static 创建全局信号处理器
        false
    }
    
    /// 阻塞直到收到关机信号
    pub async fn wait_for_shutdown_signal() {
        let ctrl_c = signal::ctrl_c();
        
        #[cfg(unix)]
        {
            let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("无法注册 SIGTERM 处理器");
            tokio::select! {
                _ = ctrl_c => {
                    crate::log_info!("收到关机信号");
                }
                _ = sigterm.recv() => {
                    crate::log_info!("收到 SIGTERM 信号");
                }
            }
        }
        
        #[cfg(not(unix))]
        {
            ctrl_c.await.expect("等待 Ctrl+C 信号失败");
            crate::log_info!("收到关机信号");
        }
    }
}