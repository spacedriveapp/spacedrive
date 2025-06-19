//! Error types for the job system

use std::fmt;
use thiserror::Error;

/// Result type for job operations
pub type JobResult<T = ()> = Result<T, JobError>;

/// Errors that can occur during job execution
#[derive(Debug, Error)]
pub enum JobError {
    /// Job was interrupted (paused or cancelled)
    #[error("Job was interrupted")]
    Interrupted,
    
    /// Job execution failed
    #[error("Job execution failed: {0}")]
    ExecutionFailed(String),
    
    /// Database operation failed
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    
    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    /// Job not found
    #[error("Job not found: {0}")]
    NotFound(String),
    
    /// Invalid job state
    #[error("Invalid job state: {0}")]
    InvalidState(String),
    
    /// Task system error
    #[error("Task system error: {0}")]
    TaskSystem(String),
    
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    /// Other errors
    #[error("{0}")]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl From<String> for JobError {
    fn from(msg: String) -> Self {
        Self::ExecutionFailed(msg)
    }
}

impl JobError {
    /// Create an execution failed error
    pub fn execution<T: fmt::Display>(msg: T) -> Self {
        Self::ExecutionFailed(msg.to_string())
    }
    
    /// Create a serialization error
    pub fn serialization<T: fmt::Display>(msg: T) -> Self {
        Self::Serialization(msg.to_string())
    }
    
    /// Create an invalid state error
    pub fn invalid_state<T: fmt::Display>(msg: T) -> Self {
        Self::InvalidState(msg.to_string())
    }
    
    /// Create a task system error
    pub fn task_system<T: fmt::Display>(msg: T) -> Self {
        Self::TaskSystem(msg.to_string())
    }
    
    /// Check if this error is due to interruption
    pub fn is_interrupted(&self) -> bool {
        matches!(self, Self::Interrupted)
    }
}

// JobError automatically implements RunError via blanket implementation