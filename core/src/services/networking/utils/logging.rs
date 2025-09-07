//! Logging utilities for networking operations

use async_trait::async_trait;

/// Trait for network logging
#[async_trait]
pub trait NetworkLogger: Send + Sync {
    async fn info(&self, message: &str);
    async fn warn(&self, message: &str);
    async fn error(&self, message: &str);
    async fn debug(&self, message: &str);
}

/// Silent logger that discards all messages
pub struct SilentLogger;

#[async_trait]
impl NetworkLogger for SilentLogger {
    async fn info(&self, _message: &str) {}
    async fn warn(&self, _message: &str) {}
    async fn error(&self, _message: &str) {}
    async fn debug(&self, _message: &str) {}
}

/// Console logger that prints to stdout/stderr
pub struct ConsoleLogger;

#[async_trait]
impl NetworkLogger for ConsoleLogger {
    async fn info(&self, message: &str) {
        println!("[NETWORKING INFO] {}", message);
    }
    
    async fn warn(&self, message: &str) {
        eprintln!("[NETWORKING WARN] {}", message);
    }
    
    async fn error(&self, message: &str) {
        eprintln!("[NETWORKING ERROR] {}", message);
    }
    
    async fn debug(&self, message: &str) {
        println!("[NETWORKING DEBUG] {}", message);
    }
}