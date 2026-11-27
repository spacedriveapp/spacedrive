//! `spaces.changes`: what changed in a space, who changed it, and how.

use std::{collections::HashMap, sync::Arc};

use sd_store::EventQuery;
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

/// Which end of a space's history a page starts from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ChangeOrder {
	/// Oldest first, for catching up from a cursor.
	#[default]
	Oldest,
	/// Newest first, for a feed or the last change to one file.
	Newest,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceChangesInput {
	pub id: Uuid,
	/// Defaults to oldest first.
	#[serde(default)]
	pub order: Option<ChangeOrder>,
	/// Oldest first: the `next` of the previous page. Omit to start at the
	/// oldest change still kept.
	#[serde(default)]
	pub since: Option<i64>,
	/// Newest first: the `next` of the previous page. Omit to start at the
	/// newest change.
	#[serde(default)]
	pub before: Option<i64>,
	/// Only changes to this path or inside it, relative to the space root
	/// and `/`-separated. A move counts for both its source and its target.
	#[serde(default)]
	pub path: Option<String>,
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
	/// Pass as `since` (oldest first) or `before` (newest first) to read
	/// the next page in the same order. It stays the same at the end of the
	/// history.
	pub next: i64,
}

/// One page of a space's changes, with each actor resolved.
pub async fn page(
	library: &Library,
	context: &CoreContext,
	source_id: Uuid,
	space_id: Uuid,
	query: EventQuery,
) -> Result<SpaceChangesOutput, String> {
	let start = query
		.cursor
		.unwrap_or(if query.newest { i64::MAX } else { 0 });
	let Some(log) = events::log_of(context, source_id, space_id).await else {
		return Ok(SpaceChangesOutput {
			changes: Vec::new(),
			next: start,
		});
	};
	let rows = log
		.read(&EventQuery {
			limit: query.limit.clamp(1, CHANGES_PAGE_MAX),
			..query
		})
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
	let next = rows.last().map_or(start, |row| row.seq);
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
		let newest = self.input.order == Some(ChangeOrder::Newest);
		page(
			&library,
			&context,
			space.source_id,
			space.id,
			EventQuery {
				cursor: if newest {
					self.input.before
				} else {
					self.input.since
				},
				newest,
				path: self.input.path,
				limit: self.input.limit.unwrap_or(100),
			},
		)
		.await
		.map_err(QueryError::Internal)
	}
}

crate::register_library_query!(SpaceChangesQuery, "spaces.changes");
