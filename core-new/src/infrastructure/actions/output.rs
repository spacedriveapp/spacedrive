//! Action execution output types

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use uuid::Uuid;

/// Trait for action outputs that can be serialized and displayed
pub trait ActionOutputTrait: std::fmt::Debug + Send + Sync {
    /// Serialize the output to JSON
    fn to_json(&self) -> serde_json::Value;
    
    /// Display the output as a human-readable string
    fn display_message(&self) -> String;
    
    /// Get the output type identifier
    fn output_type(&self) -> &'static str;
}

/// Serializable wrapper for action outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionOutput {
    /// Type identifier for the output
    pub output_type: String,
    /// JSON data for the output
    pub data: serde_json::Value,
    /// Human-readable message
    pub message: String,
}

impl ActionOutput {
    /// Create output from any type implementing ActionOutputTrait
    pub fn from_trait<T: ActionOutputTrait>(output: T) -> Self {
        Self {
            output_type: output.output_type().to_string(),
            data: output.to_json(),
            message: output.display_message(),
        }
    }
    
    /// Create a simple success output
    pub fn success(message: &str) -> Self {
        Self {
            output_type: "success".to_string(),
            data: serde_json::Value::Null,
            message: message.to_string(),
        }
    }
}

impl Default for ActionOutput {
    fn default() -> Self {
        Self::success("Action completed successfully")
    }
}

impl fmt::Display for ActionOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}