//! Permission and security system for WASM extensions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::types::ManifestPermissions;

#[derive(Error, Debug)]
pub enum PermissionError {
	#[error("Extension not authorized: {0}")]
	Unauthorized(String),

	#[error("Method not allowed: {0}")]
	MethodNotAllowed(String),

	#[error("Library access denied: {0}")]
	LibraryAccessDenied(String),

	#[error("Rate limit exceeded: {0}")]
	RateLimitExceeded(String),
}

/// Runtime permission checker with rate limiting
#[derive(Clone)]
pub struct ExtensionPermissions {
	extension_id: String,

	/// Methods this extension can call (prefix matching)
	allowed_methods: Vec<String>,

	/// Libraries this extension can access ("*" or specific UUIDs)
	allowed_libraries: Vec<String>,

	/// Rate limiting state
	rate_limiter: Arc<RwLock<RateLimiter>>,

	/// Resource limits
	pub max_memory_mb: usize,
	pub max_concurrent_jobs: usize,
}

struct RateLimiter {
	requests_per_minute: usize,
	recent_requests: Vec<Instant>,
}

impl ExtensionPermissions {
	/// Create permissions from manifest
	pub fn from_manifest(extension_id: String, manifest_perms: &ManifestPermissions) -> Self {
		Self {
			extension_id,
			allowed_methods: manifest_perms.methods.clone(),
			allowed_libraries: manifest_perms.libraries.clone(),
			rate_limiter: Arc::new(RwLock::new(RateLimiter {
				requests_per_minute: manifest_perms.rate_limits.requests_per_minute,
				recent_requests: Vec::new(),
			})),
			max_memory_mb: manifest_perms.max_memory_mb,
			max_concurrent_jobs: manifest_perms.rate_limits.concurrent_jobs,
		}
	}

	/// Check if extension can call this Wire method
	pub fn can_call(&self, method: &str) -> bool {
		// Check if any allowed prefix matches
		self.allowed_methods
			.iter()
			.any(|prefix| method.starts_with(prefix))
	}

	/// Check if extension can access this library
	pub fn can_access_library(&self, library_id: Uuid) -> bool {
		// "*" means all libraries
		if self.allowed_libraries.iter().any(|id| id == "*") {
			return true;
		}

		// Check if specific library UUID is allowed
		self.allowed_libraries
			.iter()
			.any(|id| id.parse::<Uuid>().ok() == Some(library_id))
	}

	/// Check rate limit and record request
	pub async fn check_rate_limit(&self) -> Result<(), PermissionError> {
		let mut limiter = self.rate_limiter.write().await;

		let now = Instant::now();
		let one_minute_ago = now - Duration::from_secs(60);

		// Remove requests older than 1 minute
		limiter
			.recent_requests
			.retain(|&timestamp| timestamp > one_minute_ago);

		// Check if under limit
		if limiter.recent_requests.len() >= limiter.requests_per_minute {
			return Err(PermissionError::RateLimitExceeded(format!(
				"Extension {} exceeded {} requests/minute",
				self.extension_id, limiter.requests_per_minute
			)));
		}

		// Record this request
		limiter.recent_requests.push(now);

		Ok(())
	}

	/// Full permission check for a Wire operation
	pub async fn authorize(
		&self,
		method: &str,
		library_id: Option<Uuid>,
	) -> Result<(), PermissionError> {
		// Check method permission
		if !self.can_call(method) {
			return Err(PermissionError::MethodNotAllowed(format!(
				"Extension {} not allowed to call {}",
				self.extension_id, method
			)));
		}

		// Check library access if specified
		if let Some(lib_id) = library_id {
			if !self.can_access_library(lib_id) {
				return Err(PermissionError::LibraryAccessDenied(format!(
					"Extension {} cannot access library {}",
					self.extension_id, lib_id
				)));
			}
		}

		// Check rate limit
		self.check_rate_limit().await?;

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_method_permission() {
		let perms = ExtensionPermissions {
			extension_id: "test".to_string(),
			allowed_methods: vec!["vdfs.".to_string(), "ai.ocr".to_string()],
			allowed_libraries: vec!["*".to_string()],
			rate_limiter: Arc::new(RwLock::new(RateLimiter {
				requests_per_minute: 1000,
				recent_requests: Vec::new(),
			})),
			max_memory_mb: 512,
			max_concurrent_jobs: 10,
		};

		assert!(perms.can_call("vdfs.create_entry"));
		assert!(perms.can_call("vdfs.write_sidecar"));
		assert!(perms.can_call("ai.ocr"));
		assert!(!perms.can_call("credentials.delete")); // Not allowed
	}

	#[test]
	fn test_library_permission() {
		let lib_id = Uuid::new_v4();

		let perms_all = ExtensionPermissions {
			extension_id: "test".to_string(),
			allowed_methods: vec![],
			allowed_libraries: vec!["*".to_string()],
			rate_limiter: Arc::new(RwLock::new(RateLimiter {
				requests_per_minute: 1000,
				recent_requests: Vec::new(),
			})),
			max_memory_mb: 512,
			max_concurrent_jobs: 10,
		};

		assert!(perms_all.can_access_library(lib_id));

		let perms_specific = ExtensionPermissions {
			extension_id: "test".to_string(),
			allowed_methods: vec![],
			allowed_libraries: vec![lib_id.to_string()],
			rate_limiter: Arc::new(RwLock::new(RateLimiter {
				requests_per_minute: 1000,
				recent_requests: Vec::new(),
			})),
			max_memory_mb: 512,
			max_concurrent_jobs: 10,
		};

		assert!(perms_specific.can_access_library(lib_id));
		assert!(!perms_specific.can_access_library(Uuid::new_v4()));
	}
}
