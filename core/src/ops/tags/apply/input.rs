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
	/// Tag by content identity (applies to ALL instances of this content across devices)
	/// This is the preferred/default approach
	Content(Vec<Uuid>),

	/// Tag by entry ID (applies to ONLY this specific file instance)
	/// Use when you want instance-specific tags
	Entry(Vec<i32>),
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

	/// Create an entry-scoped user tag application (tags specific instance only)
	pub fn user_tags_entry(entry_ids: Vec<i32>, tag_ids: Vec<Uuid>) -> Self {
		Self {
			targets: TagTargets::Entry(entry_ids),
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
		let target_count = match &self.targets {
			TagTargets::Content(ids) => {
				if ids.is_empty() {
					return Err("content identity IDs cannot be empty".to_string());
				}
				ids.len()
			}
			TagTargets::Entry(ids) => {
				if ids.is_empty() {
					return Err("entry IDs cannot be empty".to_string());
				}
				ids.len()
			}
		};

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
