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
