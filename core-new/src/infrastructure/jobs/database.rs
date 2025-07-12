//! Job database schema and operations
//! This is the database for the job manager, not the global library database.
//! It is used to store the job history and checkpoints with serializable data for resuming jobs.
//! The job database is not synced between devices.
//! Jobs must be dispatched by the action system if initiated by the user.

use super::{
	error::{JobError, JobResult},
	types::{JobId, JobMetrics, JobStatus},
	progress::Progress,
};
use chrono::{DateTime, Utc};
use sea_orm::{
	entity::prelude::*,
	sea_query::{Expr, Query},
	ActiveModelTrait,
	ActiveValue::Set,
	ConnectionTrait, DatabaseConnection, DbBackend, DbErr, EntityTrait, QueryFilter, Schema,
	TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::path::Path;

pub mod jobs {
	use super::*;

	/// Job record in the database
	#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
	#[sea_orm(table_name = "jobs")]
	pub struct Model {
		#[sea_orm(primary_key, auto_increment = false)]
		pub id: String,
		pub name: String,
		pub state: Vec<u8>,
		pub status: String,
		pub priority: i32,

		// Progress tracking
		pub progress_type: Option<String>,
		pub progress_data: Option<Vec<u8>>,

		// Relationships
		pub parent_job_id: Option<String>,

		// Timestamps
		pub created_at: DateTime<Utc>,
		pub started_at: Option<DateTime<Utc>>,
		pub completed_at: Option<DateTime<Utc>>,
		pub paused_at: Option<DateTime<Utc>>,

		// Error tracking
		pub error_message: Option<String>,
		pub warnings: Option<JsonValue>,
		pub non_critical_errors: Option<JsonValue>,

		// Metrics
		pub metrics: Option<Vec<u8>>,
	}

	#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
	pub enum Relation {}

	impl ActiveModelBehavior for ActiveModel {}
}

pub mod history {
	use super::*;

	/// Job history record
	#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
	#[sea_orm(table_name = "job_history")]
	pub struct Model {
		#[sea_orm(primary_key, auto_increment = false)]
		pub id: String,
		pub name: String,
		pub status: String,
		pub started_at: DateTime<Utc>,
		pub completed_at: DateTime<Utc>,
		pub duration_ms: i64,
		pub output: Option<Vec<u8>>,
		pub metrics: Option<Vec<u8>>,
	}

	#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
	pub enum Relation {}

	impl ActiveModelBehavior for ActiveModel {}
}

pub mod checkpoint {
	use super::*;

	/// Job checkpoint record
	#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
	#[sea_orm(table_name = "job_checkpoints")]
	pub struct Model {
		#[sea_orm(primary_key, auto_increment = false)]
		pub job_id: String,
		pub checkpoint_data: Vec<u8>,
		pub created_at: DateTime<Utc>,
	}

	#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
	pub enum Relation {}

	impl ActiveModelBehavior for ActiveModel {}
}

/// Initialize job database
pub async fn init_database(path: &Path) -> JobResult<DatabaseConnection> {
	// Ensure the directory exists
	tokio::fs::create_dir_all(path).await?;

	let db_path = path.join("jobs.db");
	let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

	let db = sea_orm::Database::connect(&db_url).await?;

	// Create tables
	create_tables(&db).await?;

	Ok(db)
}

/// Create job tables
async fn create_tables(db: &DatabaseConnection) -> JobResult<()> {
	let schema = Schema::new(DbBackend::Sqlite);

	// Create jobs table if not exists
	let mut jobs_statement = schema.create_table_from_entity(jobs::Entity);
	jobs_statement.if_not_exists();
	db.execute(db.get_database_backend().build(&jobs_statement))
		.await?;

	// Create history table if not exists
	let mut history_statement = schema.create_table_from_entity(history::Entity);
	history_statement.if_not_exists();
	db.execute(db.get_database_backend().build(&history_statement))
		.await?;

	// Create checkpoint table if not exists
	let mut checkpoint_statement = schema.create_table_from_entity(checkpoint::Entity);
	checkpoint_statement.if_not_exists();
	db.execute(db.get_database_backend().build(&checkpoint_statement))
		.await?;

	Ok(())
}

/// Job database operations
pub struct JobDb {
	conn: DatabaseConnection,
}

impl JobDb {
	pub fn new(conn: DatabaseConnection) -> Self {
		Self { conn }
	}

	pub fn conn(&self) -> &DatabaseConnection {
		&self.conn
	}

	/// Get all queued jobs
	pub async fn get_queued_jobs(&self) -> JobResult<Vec<jobs::Model>> {
		jobs::Entity::find()
			.filter(jobs::Column::Status.eq(JobStatus::Queued.to_string()))
			.all(&self.conn)
			.await
			.map_err(Into::into)
	}

	/// Get a job by ID
	pub async fn get_job(&self, id: JobId) -> JobResult<Option<jobs::Model>> {
		jobs::Entity::find_by_id(id.to_string())
			.one(&self.conn)
			.await
			.map_err(Into::into)
	}

	/// Update job status
	pub async fn update_status(&self, id: JobId, status: JobStatus) -> JobResult<()> {
		let mut job = jobs::ActiveModel {
			id: Set(id.to_string()),
			status: Set(status.to_string()),
			..Default::default()
		};

		// Update timestamps based on status
		match status {
			JobStatus::Running => {
				job.started_at = Set(Some(Utc::now()));
			}
			JobStatus::Paused => {
				job.paused_at = Set(Some(Utc::now()));
			}
			JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled => {
				job.completed_at = Set(Some(Utc::now()));
			}
			_ => {}
		}

		job.update(&self.conn).await?;
		Ok(())
	}

	/// Update job progress in database
	pub async fn update_progress(&self, job_id: JobId, progress: &Progress) -> JobResult<()> {
		let progress_data = rmp_serde::to_vec(progress)
			.map_err(|e| JobError::serialization(e))?;
		
		let mut job = jobs::ActiveModel {
			id: Set(job_id.to_string()),
			progress_data: Set(Some(progress_data)),
			..Default::default()
		};
		
		job.update(&self.conn).await?;
		Ok(())
	}

	/// Update job status and optionally progress atomically
	pub async fn update_status_and_progress(
		&self,
		job_id: JobId,
		status: JobStatus,
		progress: Option<&Progress>,
		error_message: Option<String>,
	) -> JobResult<()> {
		let mut job = jobs::ActiveModel {
			id: Set(job_id.to_string()),
			status: Set(status.to_string()),
			..Default::default()
		};
		
		// Update progress if provided
		if let Some(prog) = progress {
			let progress_data = rmp_serde::to_vec(prog)
				.map_err(|e| JobError::serialization(e))?;
			job.progress_data = Set(Some(progress_data));
		}
		
		// Update error message if provided
		if let Some(err_msg) = error_message {
			job.error_message = Set(Some(err_msg));
		}
		
		// Update timestamps based on status
		match status {
			JobStatus::Running => {
				job.started_at = Set(Some(Utc::now()));
			}
			JobStatus::Paused => {
				job.paused_at = Set(Some(Utc::now()));
			}
			JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled => {
				job.completed_at = Set(Some(Utc::now()));
			}
			_ => {}
		}
		
		job.update(&self.conn).await?;
		Ok(())
	}

	/// Clean up old job history
	pub async fn cleanup_history(&self, older_than: DateTime<Utc>) -> JobResult<u64> {
		let result = history::Entity::delete_many()
			.filter(history::Column::CompletedAt.lt(older_than))
			.exec(&self.conn)
			.await?;

		Ok(result.rows_affected)
	}
}
