//! Credential management operations
//!
//! Securely store and retrieve OAuth tokens, API keys, and other credentials.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::sync::Arc;

use crate::ffi::WireClient;
use crate::types::Result;

/// Credential client for secure credential management
pub struct CredentialClient {
	client: Arc<RefCell<WireClient>>,
}

impl CredentialClient {
	pub(crate) fn new(client: Arc<RefCell<WireClient>>) -> Self {
		Self { client }
	}

	/// Store a credential (encrypted by Spacedrive)
	pub fn store(&self, credential_id: &str, credential: Credential) -> Result<()> {
		self.client.borrow().call(
			"action:credentials.store.input.v1",
			&StoreCredential {
				credential_id: credential_id.to_string(),
				credential,
			},
		)
	}

	/// Get a credential (automatically refreshes OAuth if needed)
	pub fn get(&self, credential_id: &str) -> Result<Credential> {
		self.client.borrow().call(
			"query:credentials.get.v1",
			&GetCredential {
				credential_id: credential_id.to_string(),
			},
		)
	}

	/// Delete a credential
	pub fn delete(&self, credential_id: &str) -> Result<()> {
		self.client.borrow().call(
			"action:credentials.delete.input.v1",
			&DeleteCredential {
				credential_id: credential_id.to_string(),
			},
		)
	}
}

// === Types ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Credential {
	OAuth2 {
		access_token: String,
		refresh_token: Option<String>,
		expires_at: DateTime<Utc>,
		scopes: Vec<String>,
	},
	ApiKey {
		key: String,
	},
	Basic {
		username: String,
		password: String,
	},
}

impl Credential {
	/// Helper: Create OAuth2 credential
	pub fn oauth2(
		access_token: String,
		refresh_token: Option<String>,
		expires_in_seconds: i64,
		scopes: Vec<String>,
	) -> Self {
		Credential::OAuth2 {
			access_token,
			refresh_token,
			expires_at: Utc::now() + chrono::Duration::seconds(expires_in_seconds),
			scopes,
		}
	}

	/// Helper: Create API key credential
	pub fn api_key(key: String) -> Self {
		Credential::ApiKey { key }
	}
}

#[derive(Debug, Serialize, Deserialize)]
struct StoreCredential {
	credential_id: String,
	credential: Credential,
}

#[derive(Debug, Serialize, Deserialize)]
struct GetCredential {
	credential_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct DeleteCredential {
	credential_id: String,
}
