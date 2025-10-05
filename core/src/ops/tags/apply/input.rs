//! Input for apply semantic tags action

use crate::domain::tag::TagSource;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ApplyTagsInput {
	/// Entry IDs to apply tags to
	pub entry_ids: Vec<i32>,

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
	/// Create a simple user tag application
	pub fn user_tags(entry_ids: Vec<i32>, tag_ids: Vec<Uuid>) -> Self {
		Self {
			entry_ids,
			tag_ids,
			source: Some(TagSource::User),
			confidence: Some(1.0),
			applied_context: None,
			instance_attributes: None,
		}
	}

	/// Create an AI tag application with confidence
	pub fn ai_tags(
		entry_ids: Vec<i32>,
		tag_ids: Vec<Uuid>,
		confidence: f32,
		context: String,
	) -> Self {
		Self {
			entry_ids,
			tag_ids,
			source: Some(TagSource::AI),
			confidence: Some(confidence),
			applied_context: Some(context),
			instance_attributes: None,
		}
	}

	/// Validate the input
	pub fn validate(&self) -> Result<(), String> {
		if self.entry_ids.is_empty() {
			return Err("entry_ids cannot be empty".to_string());
		}

		if self.tag_ids.is_empty() {
			return Err("tag_ids cannot be empty".to_string());
		}

		if self.entry_ids.len() > 1000 {
			return Err("Cannot apply tags to more than 1000 entries at once".to_string());
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
