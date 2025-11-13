use serde::{Deserialize, Serialize};
use specta::Type;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SidecarKind {
	Thumb,
	Proxy,
	Embeddings,
	Ocr,
	Transcript,
}

impl SidecarKind {
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Thumb => "thumb",
			Self::Proxy => "proxy",
			Self::Embeddings => "embeddings",
			Self::Ocr => "ocr",
			Self::Transcript => "transcript",
		}
	}

	pub fn directory(&self) -> &'static str {
		match self {
			Self::Thumb => "thumbs",
			Self::Proxy => "proxies",
			Self::Embeddings => "embeddings",
			Self::Ocr => "ocr",
			Self::Transcript => "transcript",
		}
	}

	pub fn from_str(s: &str) -> Result<Self, String> {
		Self::try_from(s)
	}
}

impl fmt::Display for SidecarKind {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl TryFrom<&str> for SidecarKind {
	type Error = String;

	fn try_from(value: &str) -> Result<Self, Self::Error> {
		match value {
			"thumb" => Ok(Self::Thumb),
			"proxy" => Ok(Self::Proxy),
			"embeddings" => Ok(Self::Embeddings),
			"ocr" => Ok(Self::Ocr),
			"transcript" => Ok(Self::Transcript),
			_ => Err(format!("Invalid sidecar kind: {}", value)),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub struct SidecarVariant(pub String);

impl SidecarVariant {
	pub fn new(variant: impl Into<String>) -> Self {
		Self(variant.into())
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}
}

impl fmt::Display for SidecarVariant {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl From<&str> for SidecarVariant {
	fn from(value: &str) -> Self {
		Self(value.to_string())
	}
}

impl From<String> for SidecarVariant {
	fn from(value: String) -> Self {
		Self(value)
	}
}

/// Format for storing sidecar files
///
/// Format selection guidelines:
/// - Webp: Thumbnails and image derivatives (compressed images)
/// - Mp4: Video/audio proxies (standard media format)
/// - Json: Text-based structured data (OCR, transcripts)
/// - MessagePack: Binary structured data (embeddings, vectors)
/// - Text: Plain text extractions
///
/// MessagePack is preferred for embeddings because:
/// - 6x smaller than JSON (1.7KB vs 10KB per 384-dim vector)
/// - 10x faster to parse
/// - Already used in Spacedrive (job serialization)
/// - Enables sub-30ms semantic search on 1M+ files
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SidecarFormat {
	Webp,
	Mp4,
	Json,
	MessagePack,
	Text,
}

impl SidecarFormat {
	pub fn extension(&self) -> &'static str {
		match self {
			Self::Webp => "webp",
			Self::Mp4 => "mp4",
			Self::Json => "json",
			Self::MessagePack => "msgpack",
			Self::Text => "txt",
		}
	}

	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Webp => "webp",
			Self::Mp4 => "mp4",
			Self::Json => "json",
			Self::MessagePack => "messagepack",
			Self::Text => "text",
		}
	}
}

impl fmt::Display for SidecarFormat {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl TryFrom<&str> for SidecarFormat {
	type Error = String;

	fn try_from(value: &str) -> Result<Self, Self::Error> {
		match value {
			"webp" => Ok(Self::Webp),
			"mp4" => Ok(Self::Mp4),
			"json" => Ok(Self::Json),
			"msgpack" | "messagepack" => Ok(Self::MessagePack),
			"text" | "txt" => Ok(Self::Text),
			_ => Err(format!("Invalid sidecar format: {}", value)),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidecarStatus {
	Pending,
	Ready,
	Failed,
}

impl SidecarStatus {
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Pending => "pending",
			Self::Ready => "ready",
			Self::Failed => "failed",
		}
	}
}

impl fmt::Display for SidecarStatus {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl TryFrom<&str> for SidecarStatus {
	type Error = String;

	fn try_from(value: &str) -> Result<Self, Self::Error> {
		match value {
			"pending" => Ok(Self::Pending),
			"ready" => Ok(Self::Ready),
			"failed" => Ok(Self::Failed),
			_ => Err(format!("Invalid sidecar status: {}", value)),
		}
	}
}

impl TryFrom<String> for SidecarFormat {
	type Error = String;

	fn try_from(value: String) -> Result<Self, Self::Error> {
		Self::try_from(value.as_str())
	}
}
