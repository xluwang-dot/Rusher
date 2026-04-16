//! 计数器模块

use std::sync::atomic::{AtomicU32, Ordering};

/// 计数器结构体
pub struct Counter {
    /// 当前计数值
    value: AtomicU32,
    /// 最大值
    max_value: u32,
}

impl Counter {
    /// 创建一个新的计数器，最大值为24
    pub fn new() -> Self {
        Self {
            value: AtomicU32::new(0),
            max_value: 24,
        }
    }
    
    /// 创建一个新的计数器，可以指定最大值
    pub fn with_max(max_value: u32) -> Self {
        Self {
            value: AtomicU32::new(0),
            max_value,
        }
    }
    
    /// 获取当前计数值
    pub fn get(&self) -> u32 {
        self.value.load(Ordering::Relaxed)
    }
    
    /// 增加计数值
    /// 如果达到最大值，则返回false，否则返回true
    pub fn increment(&self) -> bool {
        let mut current = self.value.load(Ordering::Relaxed);
        loop {
            if current >= self.max_value {
                return false;
            }
            
            let new_value = current + 1;
            match self.value.compare_exchange_weak(
                current,
                new_value,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(actual) => current = actual,
            }
        }
    }
    
    /// 减少计数值
    /// 如果已经是0，则返回false，否则返回true
    pub fn decrement(&self) -> bool {
        let mut current = self.value.load(Ordering::Relaxed);
        loop {
            if current == 0 {
                return false;
            }
            
            let new_value = current - 1;
            match self.value.compare_exchange_weak(
                current,
                new_value,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(actual) => current = actual,
            }
        }
    }
    
    /// 重置计数器为0
    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }
    
    /// 获取最大值
    pub fn max_value(&self) -> u32 {
        self.max_value
    }
    
    /// 设置最大值
    pub fn set_max_value(&mut self, max_value: u32) {
        self.max_value = max_value;
    }
    
    /// 检查是否已达到最大值
    pub fn is_max(&self) -> bool {
        self.get() >= self.max_value
    }
    
    /// 检查是否为零
    pub fn is_zero(&self) -> bool {
        self.get() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_counter_new() {
        let counter = Counter::new();
        assert_eq!(counter.get(), 0);
        assert_eq!(counter.max_value(), 24);
        assert!(!counter.is_max());
        assert!(counter.is_zero());
    }
    
    #[test]
    fn test_counter_with_max() {
        let counter = Counter::with_max(10);
        assert_eq!(counter.get(), 0);
        assert_eq!(counter.max_value(), 10);
    }
    
    #[test]
    fn test_counter_increment() {
        let counter = Counter::with_max(5);
        
        assert!(counter.increment());
        assert_eq!(counter.get(), 1);
        
        assert!(counter.increment());
        assert_eq!(counter.get(), 2);
        
        assert!(counter.increment());
        assert_eq!(counter.get(), 3);
        
        assert!(counter.increment());
        assert_eq!(counter.get(), 4);
        
        assert!(counter.increment());
        assert_eq!(counter.get(), 5);
        
        // 已达到最大值，不能再增加
        assert!(!counter.increment());
        assert_eq!(counter.get(), 5);
        assert!(counter.is_max());
    }
    
    #[test]
    fn test_counter_decrement() {
        let counter = Counter::with_max(5);
        
        // 先增加到3
        counter.increment();
        counter.increment();
        counter.increment();
        assert_eq!(counter.get(), 3);
        
        assert!(counter.decrement());
        assert_eq!(counter.get(), 2);
        
        assert!(counter.decrement());
        assert_eq!(counter.get(), 1);
        
        assert!(counter.decrement());
        assert_eq!(counter.get(), 0);
        
        // 已经是0，不能再减少
        assert!(!counter.decrement());
        assert_eq!(counter.get(), 0);
        assert!(counter.is_zero());
    }
    
    #[test]
    fn test_counter_reset() {
        let counter = Counter::with_max(5);
        
        counter.increment();
        counter.increment();
        assert_eq!(counter.get(), 2);
        
        counter.reset();
        assert_eq!(counter.get(), 0);
        assert!(counter.is_zero());
    }
    
    #[test]
    fn test_counter_set_max_value() {
        let mut counter = Counter::with_max(5);
        assert_eq!(counter.max_value(), 5);
        
        counter.set_max_value(10);
        assert_eq!(counter.max_value(), 10);
        
        // 测试新的最大值
        for _ in 0..10 {
            assert!(counter.increment());
        }
        assert_eq!(counter.get(), 10);
        assert!(counter.is_max());
        assert!(!counter.increment());
    }
}