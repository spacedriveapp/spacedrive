//! Action context for tracking job origins
//!
//! This module provides the infrastructure to track which action spawned each job,
//! enabling rich contextual metadata throughout the job lifecycle.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;

/// Context information about the action that spawned a job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionContext {
	/// The action type that spawned this job (e.g., "locations.add", "indexing.scan")
	pub action_type: String,

	/// When the action was initiated
	pub initiated_at: DateTime<Utc>,

	/// User/session that triggered the action (if available)
	pub initiated_by: Option<String>,

	/// The original action input (sanitized for security)
	pub action_input: serde_json::Value,

	/// Additional action-specific context
	pub context: serde_json::Value,
}

impl ActionContext {
	/// Create a new action context
	pub fn new(
		action_type: impl Into<String>,
		action_input: serde_json::Value,
		context: serde_json::Value,
	) -> Self {
		Self {
			action_type: action_type.into(),
			initiated_at: Utc::now(),
			initiated_by: None,
			action_input,
			context,
		}
	}

	/// Set the user/session that initiated this action
	pub fn with_initiated_by(mut self, initiated_by: impl Into<String>) -> Self {
		self.initiated_by = Some(initiated_by.into());
		self
	}

	/// Set a custom initiation timestamp
	pub fn with_initiated_at(mut self, initiated_at: DateTime<Utc>) -> Self {
		self.initiated_at = initiated_at;
		self
	}
}

/// Trait for actions to provide context information
pub trait ActionContextProvider {
	/// Create action context for this action instance
	fn create_action_context(&self) -> ActionContext;

	/// Get the action type name (e.g., "locations.add")
	fn action_type_name() -> &'static str
	where
		Self: Sized;
}

/// Helper to safely serialize action input, removing sensitive fields if needed
pub fn sanitize_action_input<T: Serialize>(input: &T) -> serde_json::Value {
	// For now, serialize as-is. In the future, we could implement
	// field-level sanitization to remove passwords, tokens, etc.
	serde_json::to_value(input).unwrap_or_default()
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn test_action_context_creation() {
		let context = ActionContext::new(
			"test.action",
			json!({"path": "/test/path"}),
			json!({"operation": "test"}),
		);

		assert_eq!(context.action_type, "test.action");
		assert!(context.initiated_by.is_none());
		assert_eq!(context.action_input, json!({"path": "/test/path"}));
		assert_eq!(context.context, json!({"operation": "test"}));
	}

	#[test]
	fn test_action_context_with_user() {
		let context =
			ActionContext::new("test.action", json!({}), json!({})).with_initiated_by("test_user");

		assert_eq!(context.initiated_by, Some("test_user".to_string()));
	}

	#[test]
	fn test_sanitize_action_input() {
		#[derive(Serialize)]
		struct TestInput {
			path: String,
			name: Option<String>,
		}

		let input = TestInput {
			path: "/test/path".to_string(),
			name: Some("Test".to_string()),
		};

		let sanitized = sanitize_action_input(&input);
		assert_eq!(
			sanitized,
			json!({
				"path": "/test/path",
				"name": "Test"
			})
		);
	}
}
