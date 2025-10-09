//! Runtime job registry for WASM extensions
//!
//! Allows extensions to register custom job types at runtime that integrate
//! with the core job system.

use crate::infra::extension::WasmJob;
use std::collections::HashMap;
use std::sync::RwLock;

/// Metadata for a registered extension job
#[derive(Debug, Clone)]
pub struct ExtensionJobRegistration {
	/// Extension ID (e.g., "finance")
	pub extension_id: String,
	/// Job name (e.g., "email_scan")
	pub job_name: String,
	/// Full qualified name (e.g., "finance:email_scan")
	pub full_name: String,
	/// WASM export function name (e.g., "execute_email_scan")
	pub export_fn: String,
	/// Whether this job supports resumption
	pub resumable: bool,
}

/// Runtime registry for extension-defined jobs
pub struct ExtensionJobRegistry {
	/// Map from full job name (e.g., "finance:email_scan") to registration
	jobs: RwLock<HashMap<String, ExtensionJobRegistration>>,
}

impl ExtensionJobRegistry {
	/// Create a new empty registry
	pub fn new() -> Self {
		Self {
			jobs: RwLock::new(HashMap::new()),
		}
	}

	/// Register a new extension job
	pub fn register(
		&self,
		extension_id: String,
		job_name: String,
		export_fn: String,
		resumable: bool,
	) -> Result<(), String> {
		let full_name = format!("{}:{}", extension_id, job_name);

		let registration = ExtensionJobRegistration {
			extension_id: extension_id.clone(),
			job_name: job_name.clone(),
			full_name: full_name.clone(),
			export_fn: export_fn.clone(),
			resumable,
		};

		let mut jobs = self.jobs.write().unwrap();

		// Check for duplicates
		if jobs.contains_key(&full_name) {
			return Err(format!(
				"Job '{}' is already registered by extension '{}'",
				job_name, extension_id
			));
		}

		tracing::info!("Registered extension job: {} -> {}", full_name, export_fn);

		jobs.insert(full_name, registration);
		Ok(())
	}

	/// Check if a job name is registered
	pub fn has_job(&self, full_name: &str) -> bool {
		self.jobs.read().unwrap().contains_key(full_name)
	}

	/// Get registration info for a job
	pub fn get_job(&self, full_name: &str) -> Option<ExtensionJobRegistration> {
		self.jobs.read().unwrap().get(full_name).cloned()
	}

	/// Create a WasmJob instance from a registered job name
	pub fn create_wasm_job(&self, full_name: &str, state_json: String) -> Result<WasmJob, String> {
		let registration = self
			.get_job(full_name)
			.ok_or_else(|| format!("Extension job '{}' not found", full_name))?;

		Ok(WasmJob {
			extension_id: registration.extension_id,
			export_fn: registration.export_fn,
			state_json,
			is_resuming: false,
		})
	}

	/// List all registered jobs for an extension
	pub fn list_jobs_for_extension(&self, extension_id: &str) -> Vec<ExtensionJobRegistration> {
		self.jobs
			.read()
			.unwrap()
			.values()
			.filter(|reg| reg.extension_id == extension_id)
			.cloned()
			.collect()
	}

	/// List all registered extension jobs
	pub fn list_all_jobs(&self) -> Vec<ExtensionJobRegistration> {
		self.jobs.read().unwrap().values().cloned().collect()
	}

	/// Unregister all jobs for an extension (called on unload)
	pub fn unregister_extension_jobs(&self, extension_id: &str) -> usize {
		let mut jobs = self.jobs.write().unwrap();
		let before_count = jobs.len();

		jobs.retain(|_, reg| reg.extension_id != extension_id);

		let removed = before_count - jobs.len();
		if removed > 0 {
			tracing::info!(
				"Unregistered {} job(s) for extension '{}'",
				removed,
				extension_id
			);
		}

		removed
	}
}

impl Default for ExtensionJobRegistry {
	fn default() -> Self {
		Self::new()
	}
}
