//! Logging abstraction for networking components
//! 
//! This module provides trait-based logging to decouple the core networking
//! from specific logging implementations (console, file, GUI, etc.)

use async_trait::async_trait;

/// Trait for logging network events and operations
#[async_trait]
pub trait NetworkLogger: Send + Sync {
    /// Log an informational message
    async fn info(&self, message: &str);
    
    /// Log an error message
    async fn error(&self, message: &str);
    
    /// Log a debug message
    async fn debug(&self, message: &str);
    
    /// Log a warning message
    async fn warn(&self, message: &str);
    
    /// Log a trace message (very detailed)
    async fn trace(&self, message: &str);
}

/// Silent logger for testing or when logging is not needed
pub struct SilentLogger;

#[async_trait]
impl NetworkLogger for SilentLogger {
    async fn info(&self, _message: &str) {}
    async fn error(&self, _message: &str) {}
    async fn debug(&self, _message: &str) {}
    async fn warn(&self, _message: &str) {}
    async fn trace(&self, _message: &str) {}
}

/// Mock logger for testing that captures log messages
#[derive(Debug, Clone, Default)]
pub struct MockLogger {
    pub messages: std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>, // (level, message)
}

impl MockLogger {
    pub fn new() -> Self {
        Self {
            messages: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
    
    pub fn get_messages(&self) -> Vec<(String, String)> {
        self.messages.lock().unwrap().clone()
    }
    
    pub fn clear(&self) {
        self.messages.lock().unwrap().clear();
    }
}

#[async_trait]
impl NetworkLogger for MockLogger {
    async fn info(&self, message: &str) {
        self.messages.lock().unwrap().push(("INFO".to_string(), message.to_string()));
    }
    
    async fn error(&self, message: &str) {
        self.messages.lock().unwrap().push(("ERROR".to_string(), message.to_string()));
    }
    
    async fn debug(&self, message: &str) {
        self.messages.lock().unwrap().push(("DEBUG".to_string(), message.to_string()));
    }
    
    async fn warn(&self, message: &str) {
        self.messages.lock().unwrap().push(("WARN".to_string(), message.to_string()));
    }
    
    async fn trace(&self, message: &str) {
        self.messages.lock().unwrap().push(("TRACE".to_string(), message.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_silent_logger() {
        let logger = SilentLogger;
        // Should not panic or produce any output
        logger.info("test message").await;
        logger.error("error message").await;
    }
    
    #[tokio::test]
    async fn test_mock_logger() {
        let logger = MockLogger::new();
        
        logger.info("info message").await;
        logger.error("error message").await;
        logger.debug("debug message").await;
        
        let messages = logger.get_messages();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0], ("INFO".to_string(), "info message".to_string()));
        assert_eq!(messages[1], ("ERROR".to_string(), "error message".to_string()));
        assert_eq!(messages[2], ("DEBUG".to_string(), "debug message".to_string()));
        
        logger.clear();
        assert_eq!(logger.get_messages().len(), 0);
    }
}