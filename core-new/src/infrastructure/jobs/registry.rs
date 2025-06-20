//! Job registry for automatic discovery

use super::{
    error::{JobError, JobResult},
    types::{ErasedJob, JobRegistration, JobSchema},
};
use inventory;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use tracing::info;

// Inventory for auto-registration
inventory::collect!(JobRegistration);

/// Global job registry
pub struct JobRegistry {
    jobs: HashMap<&'static str, JobRegistration>,
}

impl JobRegistry {
    /// Create a new registry and discover all jobs
    pub fn new() -> Self {
        let mut jobs = HashMap::new();
        
        // Collect all registered jobs
        for registration in inventory::iter::<JobRegistration> {
            info!("Registered job: {}", registration.name);
            jobs.insert(registration.name, JobRegistration {
                name: registration.name,
                schema_fn: registration.schema_fn,
                create_fn: registration.create_fn,
                deserialize_fn: registration.deserialize_fn,
            });
        }
        
        info!("Discovered {} job types", jobs.len());
        
        Self { jobs }
    }
    
    /// Get all registered job names
    pub fn job_names(&self) -> Vec<&'static str> {
        self.jobs.keys().copied().collect()
    }
    
    /// Get schema for a job
    pub fn get_schema(&self, name: &str) -> Option<JobSchema> {
        self.jobs.get(name).map(|reg| (reg.schema_fn)())
    }
    
    /// Create a job instance from serialized data
    pub fn create_job(&self, name: &str, data: serde_json::Value) -> JobResult<Box<dyn ErasedJob>> {
        let registration = self.jobs.get(name)
            .ok_or_else(|| JobError::NotFound(format!("Job type '{}' not found", name)))?;
        
        (registration.create_fn)(data)
            .map_err(|e| JobError::serialization(e))
    }
    
    /// Deserialize a job instance from binary data (for resumption)
    pub fn deserialize_job(&self, name: &str, data: &[u8]) -> JobResult<Box<dyn ErasedJob>> {
        let registration = self.jobs.get(name)
            .ok_or_else(|| JobError::NotFound(format!("Job type '{}' not found", name)))?;
        
        (registration.deserialize_fn)(data)
            .map_err(|e| JobError::serialization(e))
    }
    
    /// Check if a job type is registered
    pub fn has_job(&self, name: &str) -> bool {
        self.jobs.contains_key(name)
    }
}

/// Global registry instance
pub static REGISTRY: Lazy<JobRegistry> = Lazy::new(JobRegistry::new);

/// Helper macro for registering jobs
/// This would be used by the derive macro
#[macro_export]
macro_rules! register_job {
    ($job_type:ty) => {
        inventory::submit! {
            $crate::infrastructure::jobs::types::JobRegistration {
                name: <$job_type as $crate::infrastructure::jobs::traits::Job>::NAME,
                schema_fn: <$job_type as $crate::infrastructure::jobs::traits::Job>::schema,
                create_fn: |data| {
                    let job: $job_type = serde_json::from_value(data)?;
                    Ok(Box::new($crate::infrastructure::jobs::executor::JobExecutor::new(job)))
                },
                deserialize_fn: |data| {
                    let job: $job_type = rmp_serde::from_slice(data)?;
                    Ok(Box::new($crate::infrastructure::jobs::executor::JobExecutor::new(job)))
                },
            }
        }
    };
}