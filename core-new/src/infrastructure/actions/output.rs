//! Action execution output types

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use uuid::Uuid;
use crate::volume::VolumeFingerprint;

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
#[serde(tag = "type", content = "data")]
pub enum ActionOutput {
    /// Simple success message
    Success { message: String },
    
    /// Volume was tracked
    VolumeTracked {
        fingerprint: VolumeFingerprint,
        library_id: Uuid,
        volume_name: String,
    },
    
    /// Volume was untracked
    VolumeUntracked {
        fingerprint: VolumeFingerprint,
        library_id: Uuid,
    },
    
    /// Volume speed test completed
    VolumeSpeedTested {
        fingerprint: VolumeFingerprint,
        read_speed_mbps: Option<u32>,
        write_speed_mbps: Option<u32>,
    },
    
    /// Generic output with custom data
    Custom {
        output_type: String,
        data: serde_json::Value,
        message: String,
    },
}

impl ActionOutput {
    /// Create output from any type implementing ActionOutputTrait
    pub fn from_trait<T: ActionOutputTrait>(output: T) -> Self {
        Self::Custom {
            output_type: output.output_type().to_string(),
            data: output.to_json(),
            message: output.display_message(),
        }
    }
    
    /// Create a simple success output
    pub fn success(message: &str) -> Self {
        Self::Success {
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
        match self {
            Self::Success { message } => write!(f, "{}", message),
            Self::VolumeTracked { volume_name, .. } => {
                write!(f, "Volume '{}' tracked successfully", volume_name)
            }
            Self::VolumeUntracked { fingerprint, .. } => {
                write!(f, "Volume {} untracked successfully", fingerprint)
            }
            Self::VolumeSpeedTested { 
                fingerprint, 
                read_speed_mbps, 
                write_speed_mbps 
            } => {
                match (read_speed_mbps, write_speed_mbps) {
                    (Some(read), Some(write)) => {
                        write!(f, "Volume {} speed test: {} MB/s read, {} MB/s write", 
                            fingerprint, read, write)
                    }
                    _ => write!(f, "Volume {} speed test completed", fingerprint),
                }
            }
            Self::Custom { message, .. } => write!(f, "{}", message),
        }
    }
}