//! File type identification system
//!
//! A modern, extensible file type identification system that combines
//! extension matching, magic bytes, and content analysis.

use crate::domain::ContentKind;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

pub mod builtin;
pub mod magic;
pub mod registry;

pub use magic::{MagicByte, MagicBytePattern};
pub use registry::FileTypeRegistry;

/// A file type definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileType {
	/// Unique identifier (e.g., "image/jpeg")
	pub id: String,

	/// Human-readable name
	pub name: String,

	/// File extensions (without dots)
	pub extensions: Vec<String>,

	/// MIME types
	pub mime_types: Vec<String>,

	/// Uniform Type Identifier (macOS)
	pub uti: Option<String>,

	/// Magic byte patterns for identification
	pub magic_bytes: Vec<MagicBytePattern>,

	/// Category for grouping
	pub category: ContentKind,

	/// Priority for conflict resolution (higher = preferred)
	pub priority: u8,

	/// Extensible metadata
	pub metadata: JsonValue,
}

/// Result of file type identification
#[derive(Debug, Clone)]
pub struct IdentificationResult {
	/// The identified file type
	pub file_type: FileType,

	/// Confidence level (0-100)
	pub confidence: u8,

	/// How it was identified
	pub method: IdentificationMethod,
}

/// How a file was identified
#[derive(Debug, Clone, Copy)]
pub enum IdentificationMethod {
	/// Identified by file extension only
	Extension,

	/// Identified by magic bytes
	MagicBytes,

	/// Identified by content analysis
	ContentAnalysis,

	/// Identified by multiple methods
	Combined,
}

/// Errors that can occur during file type identification
#[derive(Error, Debug)]
pub enum FileTypeError {
	#[error("Unknown file type")]
	UnknownType,

	#[error("Ambiguous file type: {0}")]
	AmbiguousType(String),

	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),

	#[error("Invalid configuration: {0}")]
	InvalidConfig(String),
}

pub type Result<T> = std::result::Result<T, FileTypeError>;

impl FileType {
	/// Check if this file type matches an extension
	pub fn matches_extension(&self, ext: &str) -> bool {
		self.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext))
	}

	/// Get the primary MIME type
	pub fn primary_mime_type(&self) -> Option<&str> {
		self.mime_types.first().map(|s| s.as_str())
	}

	/// Get the primary extension
	pub fn primary_extension(&self) -> Option<&str> {
		self.extensions.first().map(|s| s.as_str())
	}
}
