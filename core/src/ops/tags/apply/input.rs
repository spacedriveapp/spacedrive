//! Input for apply semantic tags action

use crate::domain::tag::TagSource;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use uuid::Uuid;

/// Specifies what to tag: content (all instances) or specific entries
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", content = "ids")]
pub enum TagTargets {
	/// Tag the bytes, which reaches every copy of them on every drive. The
	/// preferred form, and what a caller should use whenever the file has been
	/// identified.
	Content(Vec<Uuid>),

	/// Tag one file, by the uuid the volume index gave it. For a file whose
	/// bytes have not been hashed yet, and for the case where someone means
	/// this copy rather than all of them.
	File(Vec<Uuid>),
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ApplyTagsInput {
	/// What to tag: content identities or specific entries
	pub targets: TagTargets,

	/// Tag IDs to apply
	pub tag_ids: Vec<Uuid>,

	/// Source of the tag application
	pub source: Option<TagSource>,

	/// Confidence score (for AI-applied tags)
	pub confidence: Option<f32>,

	/// Context when applying (e.g., "image_analysis", "user_input")
	pub applied_context: Option<String>,

	/// Instance-specific attributes for this application
	pub instance_attributes: Option<HashMap<String, serde_json::Value>>,
}

impl TagTargets {
	/// How many things this application reaches directly. A content target
	/// counts once however many copies of the bytes exist.
	pub fn len(&self) -> usize {
		match self {
			Self::Content(ids) | Self::File(ids) => ids.len(),
		}
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// A target list that names nothing, or names a nil uuid, is a caller bug
	/// rather than an empty result.
	pub fn validate(&self) -> Result<(), String> {
		let (ids, what) = match self {
			Self::Content(ids) => (ids, "content"),
			Self::File(ids) => (ids, "file"),
		};

		if ids.is_empty() {
			return Err(format!("{what} UUIDs cannot be empty"));
		}
		if ids.iter().any(Uuid::is_nil) {
			return Err(format!("{what} UUIDs cannot contain nil values"));
		}
		Ok(())
	}
}

impl ApplyTagsInput {
	/// Create a content-scoped user tag application (tags all instances)
	pub fn user_tags_content(content_ids: Vec<Uuid>, tag_ids: Vec<Uuid>) -> Self {
		Self {
			targets: TagTargets::Content(content_ids),
			tag_ids,
			source: Some(TagSource::User),
			confidence: Some(1.0),
			applied_context: None,
			instance_attributes: None,
		}
	}

	/// Create a file-scoped user tag application (tags this copy only)
	pub fn user_tags_file(record_uuids: Vec<Uuid>, tag_ids: Vec<Uuid>) -> Self {
		Self {
			targets: TagTargets::File(record_uuids),
			tag_ids,
			source: Some(TagSource::User),
			confidence: Some(1.0),
			applied_context: None,
			instance_attributes: None,
		}
	}

	/// Create an AI tag application with confidence
	pub fn ai_tags(
		content_ids: Vec<Uuid>,
		tag_ids: Vec<Uuid>,
		confidence: f32,
		context: String,
	) -> Self {
		Self {
			targets: TagTargets::Content(content_ids),
			tag_ids,
			source: Some(TagSource::AI),
			confidence: Some(confidence),
			applied_context: Some(context),
			instance_attributes: None,
		}
	}

	/// Validate the input
	pub fn validate(&self) -> Result<(), String> {
		self.targets.validate()?;
		let target_count = self.targets.len();

		if self.tag_ids.is_empty() {
			return Err("tag_ids cannot be empty".to_string());
		}

		if target_count > 1000 {
			return Err("Cannot apply tags to more than 1000 targets at once".to_string());
		}

		if self.tag_ids.len() > 50 {
			return Err("Cannot apply more than 50 tags at once".to_string());
		}

		// Validate confidence if provided
		if let Some(confidence) = self.confidence {
			if confidence < 0.0 || confidence > 1.0 {
				return Err("confidence must be between 0.0 and 1.0".to_string());
			}
		}

		Ok(())
	}
}
