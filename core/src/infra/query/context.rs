//! Query context for tracking query origins and metadata
//!
//! This module provides the infrastructure to track which session/user initiated each query,
//! enabling rich contextual metadata throughout the query lifecycle.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Context information about the query execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryContext {
	/// The query type that was executed (e.g., "files.list", "libraries.get")
	pub query_type: String,

	/// When the query was initiated
	pub initiated_at: DateTime<Utc>,

	/// User/session that triggered the query (if available)
	pub initiated_by: Option<String>,

	/// The original query input (sanitized for security)
	pub query_input: serde_json::Value,

	/// Additional query-specific context
	pub context: serde_json::Value,

	/// Whether this query result can be cached
	pub cacheable: bool,

	/// Cache duration hint (if cacheable)
	pub cache_duration: Option<std::time::Duration>,
}

impl QueryContext {
	/// Create a new query context
	pub fn new(
		query_type: impl Into<String>,
		query_input: serde_json::Value,
		context: serde_json::Value,
	) -> Self {
		Self {
			query_type: query_type.into(),
			initiated_at: Utc::now(),
			initiated_by: None,
			query_input,
			context,
			cacheable: false,
			cache_duration: None,
		}
	}

	/// Set the user/session that initiated this query
	pub fn with_initiated_by(mut self, initiated_by: impl Into<String>) -> Self {
		self.initiated_by = Some(initiated_by.into());
		self
	}

	/// Set a custom initiation timestamp
	pub fn with_initiated_at(mut self, initiated_at: DateTime<Utc>) -> Self {
		self.initiated_at = initiated_at;
		self
	}

	/// Mark this query as cacheable
	pub fn with_caching(mut self, duration: std::time::Duration) -> Self {
		self.cacheable = true;
		self.cache_duration = Some(duration);
		self
	}
}

/// Trait for queries to provide context information
pub trait QueryContextProvider {
	/// Create query context for this query instance
	fn create_query_context(&self) -> QueryContext;

	/// Get the query type name (e.g., "files.list")
	fn query_type_name() -> &'static str
	where
		Self: Sized;

	/// Whether this query type is cacheable
	fn is_cacheable() -> bool
	where
		Self: Sized,
	{
		false
	}

	/// Cache duration for this query type (if cacheable)
	fn cache_duration() -> Option<std::time::Duration>
	where
		Self: Sized,
	{
		None
	}
}

/// Helper to safely serialize query input, removing sensitive fields if needed
pub fn sanitize_query_input<T: Serialize>(input: &T) -> serde_json::Value {
	// For now, serialize as-is. In the future, we could implement
	// field-level sanitization to remove passwords, tokens, etc.
	serde_json::to_value(input).unwrap_or_default()
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn test_query_context_creation() {
		let context = QueryContext::new(
			"test.query",
			json!({"path": "/test/path"}),
			json!({"operation": "test"}),
		);

		assert_eq!(context.query_type, "test.query");
		assert!(context.initiated_by.is_none());
		assert_eq!(context.query_input, json!({"path": "/test/path"}));
		assert_eq!(context.context, json!({"operation": "test"}));
		assert!(!context.cacheable);
	}

	#[test]
	fn test_query_context_with_user() {
		let context =
			QueryContext::new("test.query", json!({}), json!({})).with_initiated_by("test_user");

		assert_eq!(context.initiated_by, Some("test_user".to_string()));
	}

	#[test]
	fn test_query_context_with_caching() {
		let context = QueryContext::new("test.query", json!({}), json!({}))
			.with_caching(std::time::Duration::from_secs(60));

		assert!(context.cacheable);
		assert_eq!(
			context.cache_duration,
			Some(std::time::Duration::from_secs(60))
		);
	}

	#[test]
	fn test_sanitize_query_input() {
		#[derive(Serialize)]
		struct TestInput {
			path: String,
			filter: Option<String>,
		}

		let input = TestInput {
			path: "/test/path".to_string(),
			filter: Some("*.txt".to_string()),
		};

		let sanitized = sanitize_query_input(&input);
		assert_eq!(
			sanitized,
			json!({
				"path": "/test/path",
				"filter": "*.txt"
			})
		);
	}
}
