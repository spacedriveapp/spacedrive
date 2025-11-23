//! Input for create semantic tag action

use crate::domain::tag::{PrivacyLevel, TagType};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateTagInput {
	/// The canonical name for this tag
	pub canonical_name: String,

	/// Optional display name (if different from canonical)
	pub display_name: Option<String>,

	/// Semantic variants
	pub formal_name: Option<String>,
	pub abbreviation: Option<String>,
	pub aliases: Vec<String>,

	/// Context and categorization
	pub namespace: Option<String>,
	pub tag_type: Option<TagType>,

	/// Visual properties
	pub color: Option<String>,
	pub icon: Option<String>,
	pub description: Option<String>,

	/// Advanced capabilities
	pub is_organizational_anchor: Option<bool>,
	pub privacy_level: Option<PrivacyLevel>,
	pub search_weight: Option<i32>,

	/// Initial attributes
	pub attributes: Option<HashMap<String, serde_json::Value>>,

	/// Optional: Targets to immediately apply this tag to after creation
	pub apply_to: Option<ApplyToTargets>,
}

/// Targets for immediately applying a newly created tag
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", content = "ids")]
pub enum ApplyToTargets {
	/// Apply to content identities (all instances)
	Content(Vec<Uuid>),
	/// Apply to specific entries (single instance)
	Entry(Vec<i32>),
}

impl CreateTagInput {
	/// Create a simple tag input with just a name
	pub fn simple(canonical_name: String) -> Self {
		Self {
			canonical_name,
			display_name: None,
			formal_name: None,
			abbreviation: None,
			aliases: Vec::new(),
			namespace: None,
			tag_type: None,
			color: None,
			icon: None,
			description: None,
			is_organizational_anchor: None,
			privacy_level: None,
			search_weight: None,
			attributes: None,
			apply_to: None,
		}
	}

	/// Create a tag with namespace
	pub fn with_namespace(canonical_name: String, namespace: String) -> Self {
		Self {
			canonical_name,
			namespace: Some(namespace),
			..Self::simple("".to_string())
		}
	}

	/// Validate the input
	pub fn validate(&self) -> Result<(), String> {
		if self.canonical_name.trim().is_empty() {
			return Err("canonical_name cannot be empty".to_string());
		}

		if self.canonical_name.len() > 255 {
			return Err("canonical_name cannot exceed 255 characters".to_string());
		}

		// Validate namespace if provided
		if let Some(namespace) = &self.namespace {
			if namespace.trim().is_empty() {
				return Err("namespace cannot be empty if provided".to_string());
			}
			if namespace.len() > 100 {
				return Err("namespace cannot exceed 100 characters".to_string());
			}
		}

		// Validate search weight
		if let Some(weight) = self.search_weight {
			if weight < 0 || weight > 1000 {
				return Err("search_weight must be between 0 and 1000".to_string());
			}
		}

		// Validate color format (hex)
		if let Some(color) = &self.color {
			if !color.starts_with('#') || color.len() != 7 {
				return Err("color must be in hex format (#RRGGBB)".to_string());
			}
		}

		Ok(())
	}
}
