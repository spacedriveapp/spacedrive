use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::Arc,
};

use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	infra::{
		db::entities::agent_grant,
		job::{
			database::{jobs, journal},
			journal::Effect,
		},
		query::{LibraryQuery, QueryError, QueryResult},
	},
};

/// The newest journal rows read per request. Older changes fall off the
/// feed rather than make it scan a large copy job's journal on every open.
const SCAN_ROWS: u64 = 20_000;
const DEFAULT_LIMIT: u32 = 50;

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct ActivityListInput {
	/// Only changes at or under this path on this device. Every change
	/// when absent.
	#[serde(default)]
	pub path: Option<PathBuf>,
	/// The most entries to return, newest first. Defaults to 50.
	#[serde(default)]
	pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ActivityListOutput {
	pub entries: Vec<ActivityEntry>,
}

/// Who made a change.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActivityActor {
	/// An agent writing through a grant. `grant_id` is `None` for writes
	/// journaled before writes recorded their grant.
	Agent {
		grant_id: Option<Uuid>,
		label: Option<String>,
	},
	/// Someone using Spacedrive on this device, through a job such as a copy.
	Person { device: String, job: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
	Create,
	MakeDir,
	Modify,
	Move,
	Delete,
	Attributes,
}

/// One change: an agent write, or a job's whole journal.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ActivityEntry {
	/// The journal's id. `files.undo` with this id reverses the change.
	pub id: Uuid,
	pub at: DateTime<Utc>,
	pub actor: ActivityActor,
	/// What the last step of the change did, which for an agent write is
	/// the write itself rather than the folders made above it. With a
	/// requested path, the last step under it.
	pub kind: ActivityKind,
	pub path: PathBuf,
	/// Where a move went.
	pub to_path: Option<PathBuf>,
	/// How many paths the change touched, counting only those under the
	/// requested path when one was given.
	pub changes: u32,
	/// Whether `files.undo` can reverse at least part of it.
	pub undoable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityListQuery {
	input: ActivityListInput,
}

impl LibraryQuery for ActivityListQuery {
	type Input = ActivityListInput;
	type Output = ActivityListOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library = crate::ops::grants::store::session_library(&context, &session).await?;
		let jobs_db = library.jobs().database();
		let internal = |e: sea_orm::DbErr| QueryError::Internal(format!("read activity: {e}"));
		let limit = self.input.limit.unwrap_or(DEFAULT_LIMIT) as usize;
		// The inspector asks with the index's spelling while grant writes
		// journal the canonical one, so every spelling of the path counts.
		let scope = match self.input.path.clone() {
			Some(path) => Some(
				tokio::task::spawn_blocking(move || crate::config::path_spellings(&path))
					.await
					.map_err(|e| QueryError::Internal(format!("read activity: {e}")))?,
			),
			None => None,
		};

		let rows = journal::Entity::find()
			.order_by_desc(journal::Column::RecordedAt)
			.order_by_desc(journal::Column::Sequence)
			.limit(SCAN_ROWS)
			.all(jobs_db.conn())
			.await
			.map_err(internal)?;

		let mut order: Vec<String> = Vec::new();
		let mut groups: HashMap<String, Vec<journal::Model>> = HashMap::new();
		for row in rows {
			if !groups.contains_key(&row.job_id) {
				order.push(row.job_id.clone());
			}
			groups.entry(row.job_id.clone()).or_default().push(row);
		}

		let mut changes = Vec::new();
		for id in order {
			let Some(mut rows) = groups.remove(&id) else {
				continue;
			};
			let Ok(uuid) = id.parse::<Uuid>() else {
				continue;
			};
			rows.sort_by_key(|row| row.sequence);
			let effects: Vec<(DateTime<Utc>, Effect)> = rows
				.iter()
				.filter_map(|row| {
					rmp_serde::from_slice(&row.effect)
						.ok()
						.map(|effect| (row.recorded_at, effect))
				})
				.collect();
			let shown = matching(&effects, scope.as_deref());
			let Some((_, last)) = shown.last() else {
				continue;
			};
			let grant = rows
				.iter()
				.find_map(|row| row.grant_id.as_deref())
				.and_then(|id| id.parse::<Uuid>().ok());
			let (kind, path, to_path) = describe(last);
			changes.push((
				uuid,
				grant,
				ActivityEntry {
					id: uuid,
					at: shown.iter().map(|(at, _)| *at).max().unwrap_or_default(),
					actor: ActivityActor::Agent {
						grant_id: grant,
						label: None,
					},
					kind,
					path,
					to_path,
					changes: shown.len() as u32,
					undoable: effects.iter().any(|(_, effect)| effect.reversible()),
				},
			));
		}
		// A job's newest step may lie outside the requested path, so order by
		// the steps that matched before taking the newest.
		changes.sort_by(|a, b| b.2.at.cmp(&a.2.at));
		changes.truncate(limit);

		let ids: Vec<String> = changes.iter().map(|(id, _, _)| id.to_string()).collect();
		let job_names: HashMap<String, String> = jobs::Entity::find()
			.filter(jobs::Column::Id.is_in(ids))
			.all(jobs_db.conn())
			.await
			.map_err(internal)?
			.into_iter()
			.map(|job| (job.id, job.name))
			.collect();
		let grant_ids: Vec<Uuid> = changes.iter().filter_map(|(_, grant, _)| *grant).collect();
		let labels: HashMap<Uuid, Option<String>> = agent_grant::Entity::find()
			.filter(agent_grant::Column::Uuid.is_in(grant_ids))
			.all(library.db().conn())
			.await
			.map_err(internal)?
			.into_iter()
			.map(|grant| (grant.uuid, grant.label))
			.collect();
		let device = context
			.device_manager
			.config()
			.map(|config| config.name)
			.unwrap_or_default();

		let entries = changes
			.into_iter()
			.map(|(id, grant, mut entry)| {
				entry.actor = match (grant, job_names.get(&id.to_string())) {
					(None, Some(job)) => ActivityActor::Person {
						device: device.clone(),
						job: job.clone(),
					},
					(grant, _) => ActivityActor::Agent {
						grant_id: grant,
						label: grant.and_then(|id| labels.get(&id).cloned().flatten()),
					},
				};
				entry
			})
			.collect();
		Ok(ActivityListOutput { entries })
	}
}

/// The steps of one journal that changed something under any spelling in
/// `scope`, or every step when there is no scope.
fn matching<'a>(
	effects: &'a [(DateTime<Utc>, Effect)],
	scope: Option<&[PathBuf]>,
) -> Vec<&'a (DateTime<Utc>, Effect)> {
	effects
		.iter()
		.filter(|(_, effect)| {
			scope.is_none_or(|spellings| spellings.iter().any(|path| touches(effect, path)))
		})
		.collect()
}

/// Whether an effect changed something at or under `scope`.
fn touches(effect: &Effect, scope: &Path) -> bool {
	let within = |path: &Path| path.starts_with(scope);
	match effect {
		Effect::Created { path, .. }
		| Effect::Replaced { path, .. }
		| Effect::Removed { path }
		| Effect::Attributes { path, .. } => within(path),
		Effect::Moved { from, to, .. } => within(from) || within(to),
		Effect::Trashed { from, .. } => within(from),
	}
}

fn describe(effect: &Effect) -> (ActivityKind, PathBuf, Option<PathBuf>) {
	match effect {
		Effect::Created { path, subject } => {
			let kind = if subject.as_ref().is_some_and(|subject| subject.is_dir) {
				ActivityKind::MakeDir
			} else {
				ActivityKind::Create
			};
			(kind, path.clone(), None)
		}
		Effect::Replaced { path, .. } => (ActivityKind::Modify, path.clone(), None),
		Effect::Moved { from, to, .. } => (ActivityKind::Move, from.clone(), Some(to.clone())),
		Effect::Trashed { from, .. } => (ActivityKind::Delete, from.clone(), None),
		Effect::Removed { path } => (ActivityKind::Delete, path.clone(), None),
		Effect::Attributes { path, .. } => (ActivityKind::Attributes, path.clone(), None),
	}
}

crate::register_library_query!(ActivityListQuery, "activity.list");

#[cfg(test)]
mod tests {
	use std::path::{Path, PathBuf};

	use chrono::{TimeZone, Utc};

	use super::{matching, touches};
	use crate::infra::job::journal::Effect;

	#[test]
	fn a_scoped_entry_describes_only_its_own_steps() {
		let at = |s| Utc.timestamp_opt(s, 0).unwrap();
		let effects = vec![
			(
				at(1),
				Effect::Removed {
					path: "/r/a.md".into(),
				},
			),
			(
				at(2),
				Effect::Removed {
					path: "/r/b.md".into(),
				},
			),
		];
		let scope = [PathBuf::from("/r/a.md")];
		let shown = matching(&effects, Some(&scope));
		assert_eq!(shown.len(), 1);
		assert_eq!(shown[0].0, at(1));
		assert_eq!(matching(&effects, None).len(), 2);
	}

	#[test]
	fn any_spelling_of_the_path_matches() {
		let effects = vec![(
			Utc::now(),
			Effect::Removed {
				path: "/Users/me/w/a.md".into(),
			},
		)];
		let scope = [
			PathBuf::from("/System/Volumes/Data/Users/me/w/a.md"),
			PathBuf::from("/Users/me/w/a.md"),
		];
		assert_eq!(matching(&effects, Some(&scope)).len(), 1);
	}

	#[test]
	fn a_move_touches_both_ends() {
		let effect = Effect::Moved {
			from: "/a/notes/x.md".into(),
			to: "/b/x.md".into(),
			subject: None,
		};
		assert!(touches(&effect, Path::new("/a/notes")));
		assert!(touches(&effect, Path::new("/b")));
		assert!(!touches(&effect, Path::new("/a/note")));
	}
}
