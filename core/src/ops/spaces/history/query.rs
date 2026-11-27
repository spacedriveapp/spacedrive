//! `spaces.history`: the stored versions of a space, one file's or all of
//! them, newest first.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::{
		actors::Actor,
		spaces::{changes::ActorNames, events::RETENTION_DAYS, store, versioning},
	},
};

/// The most versions one page answers.
pub const HISTORY_PAGE_MAX: u32 = 500;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceHistoryInput {
	pub id: Uuid,
	/// Only this file, or everything in this folder, relative to the space
	/// root and `/`-separated. Omit for the whole space.
	#[serde(default)]
	pub path: Option<String>,
	/// The `next` of the previous page. Omit to start at the newest version.
	#[serde(default)]
	pub before: Option<i64>,
	/// Defaults to 100, at most 500.
	#[serde(default)]
	pub limit: Option<u32>,
}

/// What one path held from one moment on.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceVersion {
	/// Pass to `spaces.restore` as `version`.
	pub id: i64,
	/// Relative to the space root, `/`-separated.
	pub path: String,
	/// BLAKE3 of the content, hex. None for a delete.
	pub hash: Option<String>,
	pub size: Option<i64>,
	/// RFC 3339 modification time of the file when it was captured.
	pub modified_at: Option<String>,
	/// RFC 3339 time the version was captured.
	pub at: String,
	pub actor: Actor,
	/// `agent_write`, `app_op`, `watcher`, `restore`, or `baseline` for a
	/// file that was already there when versioning was turned on.
	pub origin: String,
	/// Whether the path was deleted here. A delete cannot be restored; an
	/// earlier version of the path can.
	pub deleted: bool,
	/// The `spaces.changes` entry that made this version, while it is kept.
	pub change_seq: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceHistoryOutput {
	/// Whether the space keeps versions. Off, the list is empty.
	pub versioning: bool,
	/// How long a version is kept unless a snapshot pins it.
	pub retention_days: u32,
	pub versions: Vec<SpaceVersion>,
	/// Pass as `before` for the next page. It stays the same at the end.
	pub next: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceHistoryQuery {
	input: SpaceHistoryInput,
}

impl LibraryQuery for SpaceHistoryQuery {
	type Input = SpaceHistoryInput;
	type Output = SpaceHistoryOutput;

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
			.ok_or(QueryError::LibraryNotFound(library_id))?;
		let space = store::find(&library, &context, self.input.id)
			.await
			.map_err(QueryError::InvalidInput)?;
		let start = self.input.before.unwrap_or(0);
		let mut output = SpaceHistoryOutput {
			versioning: space.versioning,
			retention_days: RETENTION_DAYS as u32,
			versions: Vec::new(),
			next: start,
		};
		if !space.versioning {
			return Ok(output);
		}
		let versions = versioning::open(&context, space.source_id, space.id)
			.await
			.ok_or_else(|| {
				QueryError::Internal("the space's versions are not on this device".to_string())
			})?;
		let rows = versions
			.history(
				self.input.path.as_deref(),
				self.input.before,
				self.input.limit.unwrap_or(100).clamp(1, HISTORY_PAGE_MAX),
			)
			.await
			.map_err(|e| QueryError::Internal(e.to_string()))?;
		let names = ActorNames::load(&library, &context)
			.await
			.map_err(QueryError::Internal)?;
		output.next = rows.last().map_or(start, |row| row.id);
		output.versions = rows
			.into_iter()
			.map(|row| SpaceVersion {
				actor: names.get(row.actor_id, &row.origin),
				deleted: row.blob_hash.is_none(),
				id: row.id,
				path: row.path,
				hash: row.blob_hash,
				size: row.size,
				modified_at: row.mtime,
				at: row.at,
				origin: row.origin,
				change_seq: row.event_seq,
			})
			.collect();
		Ok(output)
	}
}

crate::register_library_query!(SpaceHistoryQuery, "spaces.history");
