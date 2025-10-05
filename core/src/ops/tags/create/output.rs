//! Output for create semantic tag action

use crate::domain::tag::Tag;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateTagOutput {
	/// The created tag's UUID
	pub tag_id: Uuid,

	/// The canonical name of the created tag
	pub canonical_name: String,

	/// The namespace if specified
	pub namespace: Option<String>,

	/// Success message
	pub message: String,
}

impl CreateTagOutput {
	/// Create output from a semantic tag
	pub fn from_tag(tag: &Tag) -> Self {
		let message = match &tag.namespace {
			Some(namespace) => format!(
				"Created tag '{}' in namespace '{}'",
				tag.canonical_name, namespace
			),
			None => format!("Created tag '{}'", tag.canonical_name),
		};

		Self {
			tag_id: tag.id,
			canonical_name: tag.canonical_name.clone(),
			namespace: tag.namespace.clone(),
			message,
		}
	}

	/// Create a simple success output
	pub fn success(tag_id: Uuid, canonical_name: String, namespace: Option<String>) -> Self {
		let message = match &namespace {
			Some(ns) => format!(
				"Successfully created semantic tag '{}' in namespace '{}'",
				canonical_name, ns
			),
			None => format!("Successfully created semantic tag '{}'", canonical_name),
		};

		Self {
			tag_id,
			canonical_name,
			namespace,
			message,
		}
	}
}
