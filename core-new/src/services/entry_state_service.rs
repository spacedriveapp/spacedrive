use crate::infrastructure::jobs::manager::JobManager;
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

		// 1. Query Job System
		let _running_jobs = job_manager
			.list_jobs(Some(crate::infrastructure::jobs::types::JobStatus::Running))
			.await?;
		let mut processed_by_job: std::collections::HashSet<i32> = std::collections::HashSet::new();

		// TODO: Need to determine which entries are being processed by running jobs
		// For now, this is a placeholder - the actual implementation would need
		// to extract entry IDs from job metadata or parameters and populate
		// the processed_by_job set accordingly

		// 2. Query Database for remaining entries
		let remaining_ids: Vec<i32> = entry_ids
			.iter()
			.filter(|id| !processed_by_job.contains(id))
			.cloned()
			.collect();

		if !remaining_ids.is_empty() {
			// This query needs to be built. It must join from entry -> location -> volume
			// and also check content_identity for errors.
			let db_states = Self::get_physical_states(db, &remaining_ids).await?;
			results.extend(db_states);
		}

		// 3. Default any remaining to Available
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
