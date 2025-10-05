//! Request context and metadata types
//!
//! Provides additional context information for API requests
//! beyond just session data.

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

pub use super::session::RequestSource;

/// Request metadata for audit trails and tracking
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RequestMetadata {
	/// Unique identifier for this request
	pub request_id: Uuid,

	/// When the request was initiated
	pub timestamp: chrono::DateTime<chrono::Utc>,

	/// Source application making the request
	pub source: RequestSource,

	/// Client IP address if network request
	pub client_ip: Option<String>,

	/// User agent string if applicable
	pub user_agent: Option<String>,

	/// Additional request headers or metadata
	pub metadata: std::collections::HashMap<String, String>,
}

impl RequestMetadata {
	/// Create metadata for a CLI request
	pub fn cli_request() -> Self {
		Self {
			request_id: Uuid::new_v4(),
			timestamp: chrono::Utc::now(),
			source: RequestSource::Cli,
			client_ip: None,
			user_agent: Some("Spacedrive CLI".to_string()),
			metadata: std::collections::HashMap::new(),
		}
	}

	/// Create metadata for a Swift app request
	pub fn swift_request() -> Self {
		Self {
			request_id: Uuid::new_v4(),
			timestamp: chrono::Utc::now(),
			source: RequestSource::Swift,
			client_ip: None,
			user_agent: Some("Spacedrive macOS".to_string()),
			metadata: std::collections::HashMap::new(),
		}
	}

	/// Create metadata for a GraphQL request
	pub fn graphql_request(client_ip: Option<String>, user_agent: Option<String>) -> Self {
		Self {
			request_id: Uuid::new_v4(),
			timestamp: chrono::Utc::now(),
			source: RequestSource::GraphQL,
			client_ip,
			user_agent,
			metadata: std::collections::HashMap::new(),
		}
	}

	/// Create metadata for internal system operations
	pub fn internal_request() -> Self {
		Self {
			request_id: Uuid::new_v4(),
			timestamp: chrono::Utc::now(),
			source: RequestSource::Internal,
			client_ip: None,
			user_agent: None,
			metadata: std::collections::HashMap::new(),
		}
	}

	/// Add custom metadata to the request
	pub fn with_metadata<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
		self.metadata.insert(key.into(), value.into());
		self
	}
}
