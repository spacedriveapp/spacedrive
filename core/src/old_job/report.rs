use crate::{
	library::Library,
	object::{
		fs::{
			old_copy::OldFileCopierJobInit, old_cut::OldFileCutterJobInit,
			old_delete::OldFileDeleterJobInit, old_erase::OldFileEraserJobInit,
		},
		validation::old_validator_job::OldObjectValidatorJobInit,
	},
};

use sd_core_prisma_helpers::job_without_data;

use sd_prisma::prisma::job;
use sd_utils::db::{maybe_missing, MissingFieldError};

use std::fmt::{Display, Formatter};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::error;
use uuid::Uuid;

use super::JobError;

#[derive(Debug)]
pub enum JobReportUpdate {
	TaskCount(usize),
	CompletedTaskCount(usize),
	Message(String),
	Info(String),
	Phase(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OldJobReport {
	pub id: Uuid,
	pub name: String,
	pub action: Option<String>,
	pub data: Option<Vec<u8>>,
	// // In Typescript `any | null` is just `any` so we don't get prompted for null checks
	// // TODO(@Oscar): This will be fixed
	// #[specta(type = Option<HashMap<String, serde_json::Value>>)]
	pub metadata: Option<serde_json::Value>,
	pub errors_text: Vec<String>,

	pub created_at: Option<DateTime<Utc>>,
	pub started_at: Option<DateTime<Utc>>,
	pub completed_at: Option<DateTime<Utc>>,

	pub parent_id: Option<Uuid>,

	pub status: JobStatus,
	pub task_count: i32,
	pub completed_task_count: i32,
	pub info: String,

	pub phase: String,
	pub message: String,
	pub estimated_completion: DateTime<Utc>,
}

impl From<OldJobReport> for sd_core_heavy_lifting::job_system::report::Report {
	fn from(
		OldJobReport {
			id,
			name,
			action,
			data: _, // Not used in the new job system
			metadata,
			errors_text: _, // New job system uses type-safe errors
			created_at,
			started_at,
			completed_at,
			parent_id,
			status,
			task_count,
			completed_task_count,
			info,
			phase,
			message,
			estimated_completion,
		}: OldJobReport,
	) -> Self {
		use sd_core_heavy_lifting::{job_system::report::ReportOutputMetadata, JobName};

		let mut new_metadata = Vec::new();

		if let Some(metadata) = metadata {
			if let Some(metadata) = metadata.as_object() {
				if let Some(metadata) = metadata.get("output") {
					if let Some(metadata) = metadata.as_object() {
						if let Some(metadata) = metadata.get("init") {
							if let Ok(OldFileCopierJobInit {
								source_location_id,
								target_location_id,
								sources_file_path_ids,
								target_location_relative_directory_path,
							}) = serde_json::from_value::<OldFileCopierJobInit>(metadata.clone())
							{
								new_metadata.push(
									ReportOutputMetadata::Copier {
										source_location_id,
										target_location_id,
										sources_file_path_ids,
										target_location_relative_directory_path,
									}
									.into(),
								);
							} else if let Ok(OldFileCutterJobInit {
								source_location_id,
								target_location_id,
								sources_file_path_ids,
								target_location_relative_directory_path,
							}) =
								serde_json::from_value::<OldFileCutterJobInit>(metadata.clone())
							{
								new_metadata.push(
									ReportOutputMetadata::Mover {
										source_location_id,
										target_location_id,
										sources_file_path_ids,
										target_location_relative_directory_path,
									}
									.into(),
								);
							} else if let Ok(OldFileDeleterJobInit {
								location_id,
								file_path_ids,
							}) =
								serde_json::from_value::<OldFileDeleterJobInit>(metadata.clone())
							{
								new_metadata.push(
									ReportOutputMetadata::Deleter {
										location_id,
										file_path_ids,
									}
									.into(),
								);
							} else if let Ok(OldFileEraserJobInit {
								location_id,
								file_path_ids,
								passes,
							}) =
								serde_json::from_value::<OldFileEraserJobInit>(metadata.clone())
							{
								new_metadata.push(
									ReportOutputMetadata::Eraser {
										location_id,
										file_path_ids,
										passes: passes as u32,
									}
									.into(),
								);
							} else if let Ok(OldObjectValidatorJobInit { location, sub_path }) =
								serde_json::from_value::<OldObjectValidatorJobInit>(
									metadata.clone(),
								) {
								new_metadata.push(
									ReportOutputMetadata::FileValidator {
										location_id: location.id,
										sub_path,
									}
									.into(),
								);
							}
						}
					}
				}
			}
		}

		Self {
			id,
			name: match name.as_str() {
				"file_copier" => JobName::Copy,
				"file_cutter" => JobName::Move,
				"file_deleter" => JobName::Delete,
				"file_eraser" => JobName::Erase,
				"object_validator" => JobName::FileValidator,

				// Already implemented in the new job system
				"indexer" => JobName::Indexer,
				"file_identifier" => JobName::FileIdentifier,
				"media_processor" => JobName::MediaProcessor,

				unexpected_job => unimplemented!("Job {unexpected_job} not implemented"),
			},
			action,
			metadata: new_metadata,
			critical_error: None,
			non_critical_errors: Vec::new(),
			created_at,
			started_at,
			completed_at,
			parent_id,
			status: status.into(),
			task_count,
			completed_task_count,
			info,
			phase,
			message,
			estimated_completion,
		}
	}
}

impl Display for OldJobReport {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"Job <name='{}', uuid='{}'> {:#?}",
			self.name, self.id, self.status
		)
	}
}

// convert database struct into a resource struct
impl TryFrom<job::Data> for OldJobReport {
	type Error = MissingFieldError;

	fn try_from(data: job::Data) -> Result<Self, Self::Error> {
		Ok(Self {
			id: Uuid::from_slice(&data.id).expect("corrupted database"),
			name: maybe_missing(data.name, "job.name")?,
			action: data.action,
			data: data.data,
			metadata: data.metadata.and_then(|m| {
				serde_json::from_slice(&m).unwrap_or_else(|e| -> Option<serde_json::Value> {
					error!(?e, "Failed to deserialize job metadata;");
					None
				})
			}),
			errors_text: data
				.errors_text
				.map(|errors_str| errors_str.split("\n\n").map(str::to_string).collect())
				.unwrap_or_default(),
			created_at: data.date_created.map(DateTime::into),
			started_at: data.date_started.map(DateTime::into),
			completed_at: data.date_completed.map(DateTime::into),
			parent_id: data
				.parent_id
				.map(|id| Uuid::from_slice(&id).expect("corrupted database")),
			status: JobStatus::try_from(maybe_missing(data.status, "job.status")?)
				.expect("corrupted database"),
			task_count: data.task_count.unwrap_or(0),
			completed_task_count: data.completed_task_count.unwrap_or(0),
			info: data.info.unwrap_or_default(),
			phase: String::new(),
			message: String::new(),
			estimated_completion: data
				.date_estimated_completion
				.map_or(Utc::now(), DateTime::into),
		})
	}
}

// I despise having to write this twice, but it seems to be the only way to
// remove the data field from the struct
// would love to get this DRY'd up
impl TryFrom<job_without_data::Data> for OldJobReport {
	type Error = MissingFieldError;

	fn try_from(data: job_without_data::Data) -> Result<Self, Self::Error> {
		Ok(Self {
			id: Uuid::from_slice(&data.id).expect("corrupted database"),
			name: maybe_missing(data.name, "job.name")?,
			action: data.action,
			data: None,
			metadata: data.metadata.and_then(|m| {
				serde_json::from_slice(&m).unwrap_or_else(|e| -> Option<serde_json::Value> {
					error!(?e, "Failed to deserialize job metadata;");
					None
				})
			}),
			errors_text: data
				.errors_text
				.map(|errors_str| errors_str.split("\n\n").map(str::to_string).collect())
				.unwrap_or_default(),
			created_at: data.date_created.map(DateTime::into),
			started_at: data.date_started.map(DateTime::into),
			completed_at: data.date_completed.map(DateTime::into),
			parent_id: data
				.parent_id
				.map(|id| Uuid::from_slice(&id).expect("corrupted database")),
			status: JobStatus::try_from(maybe_missing(data.status, "job.status")?)
				.expect("corrupted database"),
			task_count: data.task_count.unwrap_or(0),
			completed_task_count: data.completed_task_count.unwrap_or(0),
			info: data.info.unwrap_or_default(),
			phase: String::new(),
			message: String::new(),
			estimated_completion: data
				.date_estimated_completion
				.map_or(Utc::now(), DateTime::into),
		})
	}
}

impl OldJobReport {
	pub fn new(uuid: Uuid, name: String) -> Self {
		Self {
			id: uuid,
			name,
			action: None,
			created_at: None,
			started_at: None,
			completed_at: None,
			status: JobStatus::Queued,
			errors_text: vec![],
			task_count: 0,
			data: None,
			metadata: None,
			parent_id: None,
			completed_task_count: 0,
			info: String::new(),
			phase: String::new(),
			message: String::new(),
			estimated_completion: Utc::now(),
		}
	}

	pub fn get_meta(&self) -> (String, Option<String>) {
		// actions are formatted like "added_location" or "added_location-1"
		let Some(action_name) = self.action.as_ref().map(|action| {
			action
				.split('-')
				.next()
				.map(str::to_string)
				.unwrap_or_default()
		}) else {
			return (self.id.to_string(), None);
		};
		// create a unique group_key, EG: "added_location-<location_id>"
		let group_key = self.parent_id.map_or_else(
			|| format!("{}-{}", action_name, &self.id),
			|parent_id| format!("{}-{}", action_name, parent_id),
		);

		(action_name, Some(group_key))
	}

	pub async fn create(&mut self, library: &Library) -> Result<(), JobError> {
		let now = Utc::now();

		library
			.db
			.job()
			.create(
				self.id.as_bytes().to_vec(),
				sd_utils::chain_optional_iter(
					[
						job::name::set(Some(self.name.clone())),
						job::action::set(self.action.clone()),
						job::data::set(self.data.clone()),
						job::date_created::set(Some(now.into())),
						job::status::set(Some(self.status as i32)),
						job::date_started::set(self.started_at.map(|d| d.into())),
						job::task_count::set(Some(1)),
						job::info::set(Some(self.info.clone())),
						job::completed_task_count::set(Some(0)),
					],
					[self
						.parent_id
						.map(|id| job::parent::connect(job::id::equals(id.as_bytes().to_vec())))],
				),
			)
			.exec()
			.await?;

		// Only setting created_at after we successfully created the job in DB
		self.created_at = Some(now);

		Ok(())
	}

	pub async fn update(&mut self, library: &Library) -> Result<(), JobError> {
		library
			.db
			.job()
			.update(
				job::id::equals(self.id.as_bytes().to_vec()),
				vec![
					job::status::set(Some(self.status as i32)),
					job::errors_text::set(
						(!self.errors_text.is_empty()).then(|| self.errors_text.join("\n\n")),
					),
					job::data::set(self.data.clone()),
					job::metadata::set(serde_json::to_vec(&self.metadata).ok()),
					job::task_count::set(Some(self.task_count)),
					job::info::set(Some(self.info.clone())),
					job::completed_task_count::set(Some(self.completed_task_count)),
					job::date_started::set(self.started_at.map(Into::into)),
					job::date_completed::set(self.completed_at.map(Into::into)),
				],
			)
			.select(job::select!({ id }))
			.exec()
			.await?;
		Ok(())
	}
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Eq, PartialEq)]
pub enum JobStatus {
	Queued = 0,
	Running = 1,
	Completed = 2,
	Canceled = 3,
	Failed = 4,
	Paused = 5,
	CompletedWithErrors = 6,
}

impl JobStatus {
	pub fn is_finished(self) -> bool {
		matches!(
			self,
			Self::Completed
				| Self::Canceled
				| Self::Paused
				| Self::Failed
				| Self::CompletedWithErrors
		)
	}
}

impl TryFrom<i32> for JobStatus {
	type Error = JobError;

	fn try_from(value: i32) -> Result<Self, Self::Error> {
		let s = match value {
			0 => Self::Queued,
			1 => Self::Running,
			2 => Self::Completed,
			3 => Self::Canceled,
			4 => Self::Failed,
			5 => Self::Paused,
			6 => Self::CompletedWithErrors,
			_ => return Err(JobError::InvalidJobStatusInt(value)),
		};

		Ok(s)
	}
}

// TODO(fogodev): this is temporary until we can get rid of the old job system
impl From<JobStatus> for sd_core_heavy_lifting::job_system::report::Status {
	fn from(value: JobStatus) -> Self {
		match value {
			JobStatus::Queued => Self::Queued,
			JobStatus::Running => Self::Running,
			JobStatus::Completed => Self::Completed,
			JobStatus::Canceled => Self::Canceled,
			JobStatus::Failed => Self::Failed,
			JobStatus::Paused => Self::Paused,
			JobStatus::CompletedWithErrors => Self::CompletedWithErrors,
		}
	}
}

pub struct JobReportBuilder {
	pub id: Uuid,
	pub name: String,
	pub action: Option<String>,
	pub metadata: Option<serde_json::Value>,
	pub parent_id: Option<Uuid>,
}

impl JobReportBuilder {
	pub fn build(self) -> OldJobReport {
		OldJobReport {
			id: self.id,
			name: self.name,
			action: self.action,
			created_at: None,
			started_at: None,
			completed_at: None,
			status: JobStatus::Queued,
			errors_text: vec![],
			task_count: 0,
			data: None,
			metadata: self.metadata,
			parent_id: self.parent_id,
			completed_task_count: 0,
			info: String::new(),
			phase: String::new(),
			message: String::new(),
			estimated_completion: Utc::now(),
		}
	}

	pub fn new(id: Uuid, name: String) -> Self {
		Self {
			id,
			name,
			action: None,
			metadata: None,
			parent_id: None,
		}
	}
}
