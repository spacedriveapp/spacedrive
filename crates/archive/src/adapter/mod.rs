//! Adapter system: trait definition, registry, sync reporting.

pub mod script;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::db::SourceDb;
use crate::error::Result;
use crate::safety::TrustTier;

/// Report returned after an adapter sync completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
	pub records_upserted: u64,
	pub records_deleted: u64,
	pub links_created: u64,
	pub links_removed: u64,
	pub duration_ms: u64,
	pub error: Option<String>,
}

/// Info about a registered adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterInfo {
	pub id: String,
	pub name: String,
	pub description: String,
	pub version: String,
	pub author: String,
	pub data_type: String,
	pub kind: AdapterKind,
	pub trust_tier: TrustTier,
	pub icon_svg: Option<String>,
	pub update_available: bool,
}

/// Whether an adapter is compiled-in or script-based.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AdapterKind {
	Native,
	Script,
}

/// Trait that all adapters implement.
pub trait Adapter: Send + Sync + 'static {
	fn id(&self) -> &str;
	fn name(&self) -> &str;
	fn data_type(&self) -> &str;
	fn description(&self) -> &str {
		""
	}
	fn version(&self) -> &str {
		"0.0.0"
	}
	fn author(&self) -> &str {
		""
	}
	fn icon_svg(&self) -> Option<&str> {
		None
	}
	fn trust_tier(&self) -> TrustTier {
		TrustTier::External
	}
	fn sync<'a>(
		&'a self,
		db: &'a SourceDb,
		config: &'a serde_json::Value,
	) -> Pin<Box<dyn Future<Output = Result<SyncReport>> + Send + 'a>>;
}

/// Registry of available adapters.
pub struct AdapterRegistry {
	adapters: RwLock<HashMap<String, Arc<dyn Adapter>>>,
}

impl AdapterRegistry {
	pub fn new() -> Self {
		Self {
			adapters: RwLock::new(HashMap::new()),
		}
	}

	pub fn register(&self, adapter: Arc<dyn Adapter>) {
		self.adapters
			.write()
			.expect("adapter registry poisoned")
			.insert(adapter.id().to_string(), adapter);
	}

	pub fn get(&self, id: &str) -> Option<Arc<dyn Adapter>> {
		self.adapters
			.read()
			.expect("adapter registry poisoned")
			.get(id)
			.cloned()
	}

	pub fn list(&self) -> Vec<AdapterInfo> {
		self.adapters
			.read()
			.expect("adapter registry poisoned")
			.values()
			.map(|a| AdapterInfo {
				id: a.id().to_string(),
				name: a.name().to_string(),
				description: a.description().to_string(),
				version: a.version().to_string(),
				author: a.author().to_string(),
				data_type: a.data_type().to_string(),
				kind: AdapterKind::Native,
				trust_tier: a.trust_tier(),
				icon_svg: a.icon_svg().map(|s| s.to_string()),
				update_available: false,
			})
			.collect()
	}
}

impl Default for AdapterRegistry {
	fn default() -> Self {
		Self::new()
	}
}

/// Result of an adapter update operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterUpdateResult {
	pub adapter_id: String,
	pub old_version: String,
	pub new_version: String,
	pub schema_changed: bool,
	pub backup_path: String,
}
