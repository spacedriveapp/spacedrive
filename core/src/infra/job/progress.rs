//! Progress reporting for jobs

use crate::infra::job::generic_progress::GenericProgress;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Progress information for a job
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Progress {
    /// Simple count-based progress
    Count { current: usize, total: usize },

    /// Percentage-based progress (0.0 to 1.0)
    Percentage(f32),

    /// Indeterminate progress with a message
    Indeterminate(String),

    /// Bytes-based progress
    Bytes { current: u64, total: u64 },

    /// Custom structured progress
    Structured(serde_json::Value),

    /// Generic progress (recommended for all jobs)
    Generic(GenericProgress),
}

impl Progress {
    /// Create count-based progress
    pub fn count(current: usize, total: usize) -> Self {
        Self::Count { current, total }
    }

    /// Create percentage progress
    pub fn percentage(value: f32) -> Self {
        Self::Percentage(value.clamp(0.0, 1.0))
    }

    /// Create indeterminate progress
    pub fn indeterminate(message: impl Into<String>) -> Self {
        Self::Indeterminate(message.into())
    }

    /// Create bytes-based progress
    pub fn bytes(current: u64, total: u64) -> Self {
        Self::Bytes { current, total }
    }

    /// Create structured progress
    pub fn structured<T: Serialize>(data: T) -> Self {
        Self::Structured(serde_json::to_value(data).unwrap_or(serde_json::Value::Null))
    }

    /// Create generic progress
    pub fn generic(progress: GenericProgress) -> Self {
        Self::Generic(progress)
    }

    /// Get progress as a percentage (0.0 to 1.0)
    pub fn as_percentage(&self) -> Option<f32> {
        match self {
            Self::Count { current, total } if *total > 0 => {
                Some(*current as f32 / *total as f32)
            }
            Self::Percentage(p) => Some(*p),
            Self::Bytes { current, total } if *total > 0 => {
                Some(*current as f32 / *total as f32)
            }
            Self::Generic(progress) => Some(progress.as_percentage()),
            _ => None,
        }
    }

    /// Check if progress is determinate
    pub fn is_determinate(&self) -> bool {
        !matches!(self, Self::Indeterminate(_))
    }
}

impl fmt::Display for Progress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Count { current, total } => write!(f, "{}/{}", current, total),
            Self::Percentage(p) => write!(f, "{:.1}%", p * 100.0),
            Self::Indeterminate(msg) => write!(f, "{}", msg),
            Self::Bytes { current, total } => {
                write!(f, "{}/{}", format_bytes(*current), format_bytes(*total))
            }
            Self::Structured(_) => write!(f, "[structured progress]"),
            Self::Generic(progress) => write!(f, "{}", progress.format_progress()),
        }
    }
}

/// Trait for custom progress types
pub trait JobProgress: Serialize + Send + Sync + 'static {
    /// Convert to generic Progress
    fn to_progress(&self) -> Progress {
        Progress::structured(self)
    }
}

// Helper function to format bytes
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", size as u64, UNITS[unit_idx])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}