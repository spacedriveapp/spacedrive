use crate::infrastructure::jobs::{manager::JobManager, traits::Resourceful};
use crate::operations::entries::state::EntryState;
use sea_orm::DbConn;
use std::collections::HashMap;
use uuid::Uuid;

pub struct EntryStateService;

impl EntryStateService {
	pub async fn get_states_for_entries(
		db: &DbConn,
		job_manager: &JobManager,
		entry_ids: &[i32],
	) -> Result<HashMap<i32, EntryState>, anyhow::Error> {
		let mut results = HashMap::new();

		// 1. Find all jobs that affect ANY of the requested entries.
		let affecting_jobs = job_manager.find_jobs_affecting_entries(entry_ids).await?;

		// 2. For each job, get its specific resources and update the state map.
		for job_info in affecting_jobs {
			if let Ok(job_instance) = job_manager
				.get_job_data_for_resourceful_check(&job_info.id)
				.await
			{
				// Check if the job tracks specific resources
				if let Some(affected_resources) = job_instance.try_get_affected_resources() {
					let state = Self::state_from_job(&job_info);

					for entry_id in affected_resources {
						// Only update the state for entries we were asked about.
						if entry_ids.contains(&entry_id) {
							results.insert(entry_id, state.clone());
						}
					}
				}
				// Jobs that don't track resources (return None) are skipped
			}
		}

		// 3. Query the database for the physical state of all entries
		//    that were NOT affected by a running job.
		let remaining_ids: Vec<i32> = entry_ids
			.iter()
			.filter(|id| !results.contains_key(id))
			.cloned()
			.collect();

		if !remaining_ids.is_empty() {
			// This query needs to be built. It must join from entry -> location -> volume
			// and also check content_identity for errors.
			let db_states = Self::get_physical_states(db, &remaining_ids).await?;
			results.extend(db_states);
		}

		// 4. Default any remaining entries to Available.
		for id in entry_ids {
			results.entry(*id).or_insert(EntryState::Available);
		}

		Ok(results)
	}

	// Helper to get physical states from DB
	async fn get_physical_states(
		db: &DbConn,
		ids: &[i32],
	) -> Result<HashMap<i32, EntryState>, anyhow::Error> {
		// TODO: Implement the complex SQL query here.
		// This will require joining across multiple tables.
		// For now, return an empty map.
		Ok(HashMap::new())
	}

	// Helper to map a job to a state
	fn state_from_job(job: &crate::infrastructure::jobs::types::JobInfo) -> EntryState {
		match job.name.as_str() {
			"indexer" => EntryState::Processing { job_id: job.id },
			"file_sync" => EntryState::Syncing { job_id: job.id },
			"validation" => EntryState::Validating { job_id: job.id },
			_ => EntryState::Processing { job_id: job.id }, // Default for other jobs
		}
	}
}
