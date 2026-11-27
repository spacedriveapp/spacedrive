//! `spaces.changes`: what changed in a space, who changed it, and how.

use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	infra::query::{LibraryQuery, QueryError, QueryResult},
	library::Library,
	ops::{
		actors::{self, Actor, ActorKind},
		spaces::{events, store},
	},
};

/// The most changes one page answers.
pub const CHANGES_PAGE_MAX: u32 = 500;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceChangesInput {
	pub id: Uuid,
	/// The `next` of the previous page. Omit to start at the oldest
	/// change still kept.
	#[serde(default)]
	pub since: Option<i64>,
	/// Defaults to 100, at most 500.
	#[serde(default)]
	pub limit: Option<u32>,
}

/// One change inside a space.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceChange {
	pub seq: i64,
	/// RFC 3339.
	pub at: String,
	pub actor: Actor,
	pub grant_id: Option<Uuid>,
	/// `agent_write`, `app_op` or `watcher`.
	pub origin: String,
	/// `create`, `mkdir`, `modify`, `move` or `delete`.
	pub op: String,
	/// Relative to the space root, `/`-separated.
	pub path: String,
	pub to_path: Option<String>,
	pub before_hash: Option<String>,
	pub after_hash: Option<String>,
	pub size: Option<i64>,
	/// The job whose undo reverses the change.
	pub undo_id: Option<Uuid>,
	/// One line for a person, such as "changed outside Spacedrive on Studio".
	pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceChangesOutput {
	pub changes: Vec<SpaceChange>,
	/// Pass as `since` to read the changes after this page.
	pub next: i64,
}

/// One page of a space's changes, oldest first, with each actor resolved.
pub async fn page(
	library: &Library,
	context: &CoreContext,
	source_id: Uuid,
	space_id: Uuid,
	since: i64,
	limit: u32,
) -> Result<SpaceChangesOutput, String> {
	let Some(log) = events::log_of(context, source_id, space_id).await else {
		return Ok(SpaceChangesOutput {
			changes: Vec::new(),
			next: since,
		});
	};
	let rows = log
		.since(since, limit.clamp(1, CHANGES_PAGE_MAX))
		.await
		.map_err(|e| e.to_string())?;
	// The watcher attributes to this device, so make sure it has a name.
	let _ = actors::this_device(library, context).await;
	let known: HashMap<Uuid, Actor> = actors::all(library)
		.await
		.map_err(|e| e.to_string())?
		.into_iter()
		.map(|actor| (actor.id, actor))
		.collect();
	let next = rows.last().map_or(since, |row| row.seq);
	let changes = rows
		.into_iter()
		.map(|row| {
			let actor = known.get(&row.actor_id).cloned().unwrap_or(Actor {
				id: row.actor_id,
				kind: if row.origin == events::Origin::Watcher.as_str() {
					ActorKind::Device
				} else {
					ActorKind::Agent
				},
				label: "an unknown actor".to_string(),
				harness: None,
			});
			let target = row.to_path.as_deref().unwrap_or(&row.path);
			let summary = if row.origin == events::Origin::Watcher.as_str() {
				format!("{target} changed outside Spacedrive on {}", actor.label)
			} else {
				format!("{} by {}: {target}", row.op, actor.label)
			};
			SpaceChange {
				seq: row.seq,
				at: row.at,
				actor,
				grant_id: row.grant_id,
				origin: row.origin,
				op: row.op,
				path: row.path,
				to_path: row.to_path,
				before_hash: row.before_hash,
				after_hash: row.after_hash,
				size: row.size,
				undo_id: row.undo_id,
				summary,
			}
		})
		.collect();
	Ok(SpaceChangesOutput { changes, next })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceChangesQuery {
	input: SpaceChangesInput,
}

impl LibraryQuery for SpaceChangesQuery {
	type Input = SpaceChangesInput;
	type Output = SpaceChangesOutput;

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
		page(
			&library,
			&context,
			space.source_id,
			space.id,
			self.input.since.unwrap_or(0),
			self.input.limit.unwrap_or(100),
		)
		.await
		.map_err(QueryError::Internal)
	}
}

crate::register_library_query!(SpaceChangesQuery, "spaces.changes");
