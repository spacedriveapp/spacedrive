//! Job command handlers

use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

use crate::infra::cli::daemon::services::StateService;
use crate::infra::cli::daemon::types::{DaemonCommand, DaemonResponse, JobInfo};
use crate::Core;

use super::CommandHandler;

/// Handler for job commands
pub struct JobHandler;

#[async_trait]
impl CommandHandler for JobHandler {
	async fn handle(
		&self,
		cmd: DaemonCommand,
		core: &Arc<Core>,
		state_service: &Arc<StateService>,
	) -> DaemonResponse {
		match cmd {
			DaemonCommand::ListJobs { status } => {
				// Get current library from CLI state
				if let Some(library) = state_service.get_current_library(core).await {
					let job_manager = library.jobs();

					// For running jobs, get from memory for live monitoring
					if let Some(ref status_str) = status {
						if status_str == "running" {
							let running_jobs = job_manager.list_running_jobs().await;
							let infos: Vec<JobInfo> = running_jobs
								.into_iter()
								.map(|j| JobInfo {
									id: j.id,
									name: j.name,
									status: j.status.to_string(),
									progress: j.progress,
								})
								.collect();

							return DaemonResponse::Jobs(infos);
						}
					}

					// For other statuses, query the database
					let status_filter = status.and_then(|s| {
						s.parse::<crate::infra::jobs::types::JobStatus>()
							.ok()
					});

					match job_manager.list_jobs(status_filter).await {
						Ok(jobs) => {
							let infos: Vec<JobInfo> = jobs
								.into_iter()
								.map(|j| JobInfo {
									id: j.id,
									name: j.name,
									status: j.status.to_string(),
									progress: j.progress,
								})
								.collect();

							DaemonResponse::Jobs(infos)
						}
						Err(e) => DaemonResponse::Error(e.to_string()),
					}
				} else {
					DaemonResponse::Error("No library selected".to_string())
				}
			}

			DaemonCommand::GetJobInfo { id } => {
				// Get current library from CLI state
				if let Some(library) = state_service.get_current_library(core).await {
					let job_manager = library.jobs();

					match job_manager.get_job_info(id).await {
						Ok(job) => DaemonResponse::JobInfo(job.map(|j| JobInfo {
							id: j.id,
							name: j.name,
							status: j.status.to_string(),
							progress: j.progress,
						})),
						Err(e) => DaemonResponse::Error(e.to_string()),
					}
				} else {
					DaemonResponse::Error("No library selected".to_string())
				}
			}

			DaemonCommand::PauseJob { id } => {
				// Get current library from CLI state
				if let Some(library) = state_service.get_current_library(core).await {
					let job_manager = library.jobs();
					let job_id = crate::infra::jobs::types::JobId(id);

					match job_manager.pause_job(job_id).await {
						Ok(_) => DaemonResponse::Ok,
						Err(e) => DaemonResponse::Error(e.to_string()),
					}
				} else {
					DaemonResponse::Error("No library selected".to_string())
				}
			}

			DaemonCommand::ResumeJob { id } => {
				// Get current library from CLI state
				if let Some(library) = state_service.get_current_library(core).await {
					let job_manager = library.jobs();
					let job_id = crate::infra::jobs::types::JobId(id);

					match job_manager.resume_job(job_id).await {
						Ok(_) => DaemonResponse::Ok,
						Err(e) => DaemonResponse::Error(e.to_string()),
					}
				} else {
					DaemonResponse::Error("No library selected".to_string())
				}
			}

			DaemonCommand::CancelJob { id } => {
				// TODO: Implement job cancel when job manager supports it
				DaemonResponse::Error("Job cancel not yet implemented".to_string())
			}

			_ => DaemonResponse::Error("Invalid command for job handler".to_string()),
		}
	}

	fn can_handle(&self, cmd: &DaemonCommand) -> bool {
		matches!(
			cmd,
			DaemonCommand::ListJobs { .. }
				| DaemonCommand::GetJobInfo { .. }
				| DaemonCommand::PauseJob { .. }
				| DaemonCommand::ResumeJob { .. }
				| DaemonCommand::CancelJob { .. }
		)
	}
}
