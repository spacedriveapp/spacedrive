use crate::NonCriticalError;

use sd_prisma::prisma::{file_path, job, location, PrismaClient};
use sd_utils::db::{maybe_missing, MissingFieldError};

use std::{collections::HashMap, fmt, path::PathBuf, str::FromStr};

use chrono::{DateTime, Utc};
use prisma_client_rust::QueryError;
use serde::{Deserialize, Serialize};
use specta::Type;
use strum::ParseError;

use super::{job::JobName, JobId};

#[derive(thiserror::Error, Debug)]
pub enum ReportError {
	#[error("failed to create job report in database: {0}")]
	Create(QueryError),
	#[error("failed to update job report in database: {0}")]
	Update(QueryError),
	#[error("invalid job status integer: {0}")]
	InvalidJobStatusInt(i32),
	#[error("job not found in database: <id='{0}'>")]
	MissingReport(JobId),
	#[error("json error: {0}")]
	Json(#[from] serde_json::Error),
	#[error(transparent)]
	MissingField(#[from] MissingFieldError),
	#[error("failed to parse job name from database: {0}")]
	JobNameParse(#[from] ParseError),
}

impl From<ReportError> for rspc::Error {
	fn from(e: ReportError) -> Self {
		match e {
			ReportError::Create(_)
			| ReportError::Update(_)
			| ReportError::InvalidJobStatusInt(_) => {
				Self::with_cause(rspc::ErrorCode::BadRequest, e.to_string(), e)
			}

			ReportError::MissingReport(_) => {
				Self::with_cause(rspc::ErrorCode::NotFound, e.to_string(), e)
			}
			ReportError::Json(_) | ReportError::MissingField(_) | ReportError::JobNameParse(_) => {
				Self::with_cause(rspc::ErrorCode::InternalServerError, e.to_string(), e)
			}
		}
	}
}

#[derive(Debug, Serialize, Deserialize, Type, Clone)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type", content = "metadata")]
pub enum ReportMetadata {
	Input(ReportInputMetadata),
	Output(ReportOutputMetadata),
}

#[derive(Debug, Serialize, Deserialize, Type, Clone)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type", content = "data")]
pub enum ReportInputMetadata {
	// TODO: Add more variants as needed
	Location(location::Data),
	SubPath(PathBuf),
}

#[derive(Debug, Serialize, Deserialize, Type, Clone)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type", content = "data")]
pub enum ReportOutputMetadata {
	Metrics(HashMap<String, serde_json::Value>),
	Indexer {
		total_paths: (u32, u32),
	},
	FileIdentifier {
		total_orphan_paths: (u32, u32),
		total_objects_created: (u32, u32),
		total_objects_linked: (u32, u32),
	},
	MediaProcessor {
		media_data_extracted: (u32, u32),
		media_data_skipped: (u32, u32),
		thumbnails_generated: (u32, u32),
		thumbnails_skipped: (u32, u32),
	},
	Copier {
		source_location_id: location::id::Type,
		target_location_id: location::id::Type,
		sources_file_path_ids: Vec<file_path::id::Type>,
		target_location_relative_directory_path: PathBuf,
	},
	Mover {
		source_location_id: location::id::Type,
		target_location_id: location::id::Type,
		sources_file_path_ids: Vec<file_path::id::Type>,
		target_location_relative_directory_path: PathBuf,
	},
	Deleter {
		location_id: location::id::Type,
		file_path_ids: Vec<file_path::id::Type>,
	},
	Eraser {
		location_id: location::id::Type,
		file_path_ids: Vec<file_path::id::Type>,
		passes: u32,
	},
	FileValidator {
		location_id: location::id::Type,
		sub_path: Option<PathBuf>,
	},
}

impl From<ReportInputMetadata> for ReportMetadata {
	fn from(value: ReportInputMetadata) -> Self {
		Self::Input(value)
	}
}

impl From<ReportOutputMetadata> for ReportMetadata {
	fn from(value: ReportOutputMetadata) -> Self {
		Self::Output(value)
	}
}

#[derive(Debug, Serialize, Type, Clone)]
pub struct Report {
	pub id: JobId,
	pub name: JobName,
	pub action: Option<String>,

	pub metadata: Vec<ReportMetadata>,
	pub critical_error: Option<String>,
	pub non_critical_errors: Vec<NonCriticalError>,

	pub created_at: Option<DateTime<Utc>>,
	pub started_at: Option<DateTime<Utc>>,
	pub completed_at: Option<DateTime<Utc>>,

	pub parent_id: Option<JobId>,

	pub status: Status,
	pub task_count: i32,
	pub completed_task_count: i32,
	pub info: String,

	pub phase: String,
	pub message: String,
	pub estimated_completion: DateTime<Utc>,
}

impl fmt::Display for Report {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			f,
			"Job <name='{}', uuid='{}'> {:#?}",
			self.name, self.id, self.status
		)
	}
}

// convert database struct into a resource struct
impl TryFrom<job::Data> for Report {
	type Error = ReportError;

	fn try_from(
		job::Data {
			id,
			name,
			action,
			status,
			errors_text: _, // Deprecated
			critical_error,
			non_critical_errors,
			data: _, // Deprecated
			metadata,
			parent_id,
			task_count,
			info,
			completed_task_count,
			date_estimated_completion,
			date_created,
			date_started,
			date_completed,
			..
		}: job::Data,
	) -> Result<Self, Self::Error> {
		Ok(Self {
			id: JobId::from_slice(&id).expect("corrupted database"),
			name: JobName::from_str(&maybe_missing(name, "job.name")?)?,
			action,
			metadata: if let Some(metadata) = metadata {
				serde_json::from_slice(&metadata)?
			} else {
				vec![]
			},
			critical_error,
			non_critical_errors: if let Some(non_critical_errors) = non_critical_errors {
				serde_json::from_slice(&non_critical_errors)?
			} else {
				vec![]
			},
			created_at: date_created.map(DateTime::into),
			started_at: date_started.map(DateTime::into),
			completed_at: date_completed.map(DateTime::into),
			parent_id: parent_id.map(|id| JobId::from_slice(&id).expect("corrupted database")),
			status: Status::try_from(maybe_missing(status, "job.status")?)
				.expect("corrupted database"),
			task_count: task_count.unwrap_or(0),
			completed_task_count: completed_task_count.unwrap_or(0),
			info: info.unwrap_or_default(),
			phase: String::new(),
			message: String::new(),
			estimated_completion: date_estimated_completion.map_or_else(Utc::now, DateTime::into),
		})
	}
}

impl Report {
	#[must_use]
	pub fn new(uuid: JobId, name: JobName) -> Self {
		Self {
			id: uuid,
			name,
			action: None,
			created_at: None,
			started_at: None,
			completed_at: None,
			status: Status::Queued,
			critical_error: None,
			non_critical_errors: vec![],
			task_count: 0,
			metadata: vec![],
			parent_id: None,
			completed_task_count: 0,
			info: String::new(),
			phase: String::new(),
			message: String::new(),
			estimated_completion: Utc::now(),
		}
	}

	pub fn push_metadata(&mut self, metadata: ReportOutputMetadata) {
		self.metadata.push(metadata.into());
	}

	#[must_use]
	pub fn get_action_name_and_group_key(&self) -> (String, Option<String>) {
		// actions are formatted like "added_location" or "added_location-1"
		let Some(action_name) = self
			.action
			.as_ref()
			.and_then(|action| action.split('-').next().map(str::to_string))
		else {
			return (self.id.to_string(), None);
		};
		// create a unique group_key, EG: "added_location-<location_id>"
		let group_key = self.parent_id.map_or_else(
			|| format!("{action_name}-{}", self.id),
			|parent_id| format!("{action_name}-{parent_id}"),
		);

		(action_name, Some(group_key))
	}

	pub async fn create(
		&mut self,
		db: &PrismaClient,
		created_at: DateTime<Utc>,
	) -> Result<(), ReportError> {
		db.job()
			.create(
				self.id.as_bytes().to_vec(),
				sd_utils::chain_optional_iter(
					[
						job::name::set(Some(self.name.to_string())),
						job::action::set(self.action.clone()),
						job::date_created::set(Some(created_at.into())),
						job::metadata::set(Some(serde_json::to_vec(&self.metadata)?)),
						job::status::set(Some(self.status as i32)),
						job::date_started::set(self.started_at.map(Into::into)),
						job::task_count::set(Some(0)),
						job::info::set(Some(self.info.clone())),
						job::completed_task_count::set(Some(0)),
					],
					[self
						.parent_id
						.map(|id| job::parent::connect(job::id::equals(id.as_bytes().to_vec())))],
				),
			)
			.select(job::select!({ id }))
			.exec()
			.await
			.map_err(ReportError::Create)?;

		// Only setting created_at after we successfully created the job in DB
		self.created_at = Some(created_at);

		Ok(())
	}

	pub async fn update(&self, db: &PrismaClient) -> Result<(), ReportError> {
		db.job()
			.update(
				job::id::equals(self.id.as_bytes().to_vec()),
				vec![
					job::status::set(Some(self.status as i32)),
					job::critical_error::set(self.critical_error.clone()),
					job::non_critical_errors::set(Some(serde_json::to_vec(
						&self.non_critical_errors,
					)?)),
					job::metadata::set(Some(serde_json::to_vec(&self.metadata)?)),
					job::task_count::set(Some(self.task_count)),
					job::info::set(Some(self.info.clone())),
					job::completed_task_count::set(Some(self.completed_task_count)),
					job::date_started::set(self.started_at.map(Into::into)),
					job::date_completed::set(self.completed_at.map(Into::into)),
				],
			)
			.select(job::select!({ id }))
			.exec()
			.await
			.map_err(ReportError::Update)?;

		Ok(())
	}
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Eq, PartialEq)]
pub enum Status {
	Queued = 0,
	Running = 1,
	Completed = 2,
	Canceled = 3,
	Failed = 4,
	Paused = 5,
	CompletedWithErrors = 6,
}

impl Status {
	#[must_use]
	pub const fn is_finished(self) -> bool {
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

impl TryFrom<i32> for Status {
	type Error = ReportError;

	fn try_from(value: i32) -> Result<Self, Self::Error> {
		let s = match value {
			0 => Self::Queued,
			1 => Self::Running,
			2 => Self::Completed,
			3 => Self::Canceled,
			4 => Self::Failed,
			5 => Self::Paused,
			6 => Self::CompletedWithErrors,
			_ => return Err(Self::Error::InvalidJobStatusInt(value)),
		};

		Ok(s)
	}
}

pub struct ReportBuilder {
	pub id: JobId,
	pub name: JobName,
	pub action: Option<String>,
	pub metadata: Vec<ReportMetadata>,
	pub parent_id: Option<JobId>,
}

impl ReportBuilder {
	#[must_use]
	pub fn build(self) -> Report {
		Report {
			id: self.id,
			name: self.name,
			action: self.action,
			created_at: None,
			started_at: None,
			completed_at: None,
			status: Status::Queued,
			critical_error: None,
			task_count: 0,
			non_critical_errors: vec![],
			metadata: self.metadata,
			parent_id: self.parent_id,
			completed_task_count: 0,
			info: String::new(),
			phase: String::new(),
			message: String::new(),
			estimated_completion: Utc::now(),
		}
	}

	#[must_use]
	pub const fn new(id: JobId, name: JobName) -> Self {
		Self {
			id,
			name,
			action: None,
			metadata: vec![],
			parent_id: None,
		}
	}

	#[must_use]
	pub fn with_action(mut self, action: impl Into<String>) -> Self {
		self.action = Some(action.into());
		self
	}

	#[must_use]
	pub fn with_metadata(mut self, metadata: ReportInputMetadata) -> Self {
		self.metadata.push(metadata.into());
		self
	}

	#[must_use]
	pub const fn with_parent_id(mut self, parent_id: JobId) -> Self {
		self.parent_id = Some(parent_id);
		self
	}
}
