use std::{path::PathBuf, sync::Arc};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	infra::{
		job::journal::Effect,
		query::{LibraryQuery, QueryError, QueryResult},
	},
	ops::files::trash::is_spacedrive_trash,
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TrashListInput {
	/// The most items to list; every item when absent.
	pub limit: Option<u32>,
}

/// One item a job put in the trash.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TrashedItem {
	pub job: Uuid,
	pub sequence: i64,
	/// Where it was.
	pub from: PathBuf,
	/// Where it is.
	pub location: PathBuf,
	pub trashed_at: DateTime<Utc>,
	pub size: u64,
	pub is_dir: bool,
	/// Whether the item is still at its location.
	pub present: bool,
	/// Whether it sits in a Spacedrive trash directory rather than the
	/// platform's trash.
	pub spacedrive_trash: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TrashListOutput {
	pub items: Vec<TrashedItem>,
}

pub struct TrashListQuery {
	input: TrashListInput,
}

impl LibraryQuery for TrashListQuery {
	type Input = TrashListInput;
	type Output = TrashListOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library selected".to_string()))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::LibraryNotFound(library_id))?;
		let trashed = library
			.jobs()
			.database()
			.trashed()
			.await
			.map_err(|e| QueryError::Internal(e.to_string()))?;
		let mut items = Vec::new();
		for (job, recorded) in trashed {
			let Effect::Trashed {
				from,
				to: Some(location),
				subject,
			} = recorded.effect
			else {
				continue;
			};
			let Ok(job) = job.parse::<Uuid>() else {
				continue;
			};
			let present = tokio::fs::symlink_metadata(&location).await.is_ok();
			items.push(TrashedItem {
				job,
				sequence: recorded.sequence,
				spacedrive_trash: is_spacedrive_trash(&location),
				from,
				location,
				trashed_at: recorded.recorded_at,
				size: subject.map(|subject| subject.size).unwrap_or(0),
				is_dir: subject.is_some_and(|subject| subject.is_dir),
				present,
			});
			if self
				.input
				.limit
				.is_some_and(|limit| items.len() >= limit as usize)
			{
				break;
			}
		}
		Ok(TrashListOutput { items })
	}
}

crate::register_library_query!(TrashListQuery, "files.trash_list");
