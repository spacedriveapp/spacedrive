//! `spaces.snapshot.*`: name the state of a space with versioning on, list
//! the names, and remove them.
//!
//! A snapshot pins the manifest it names, so every version it holds is kept
//! past retention until the snapshot is deleted. Creating one copies no
//! bytes.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	domain::Space,
	infra::{
		action::{error::ActionError, LibraryAction},
		query::{LibraryQuery, QueryError, QueryResult},
	},
	library::Library,
	ops::{
		actors::{self, Actor},
		spaces::{changes::ActorNames, store, versioning},
	},
};

/// A named state of a space.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceSnapshot {
	pub id: Uuid,
	pub name: String,
	/// The manifest it pins: the newest version id when it was taken.
	pub manifest_id: i64,
	pub created_by: Actor,
	/// RFC 3339.
	pub at: String,
	/// Files in the snapshot and their total size.
	pub files: u64,
	pub bytes: u64,
}

/// A space's versions. Creating a snapshot needs versioning on; listing and
/// deleting work on versions kept after it was turned off.
async fn versions_of(
	library: &Library,
	context: &CoreContext,
	id: Uuid,
	creating: bool,
) -> Result<(Space, Arc<sd_store::SpaceVersions>), String> {
	let space = store::find(library, context, id).await?;
	if creating && !space.versioning {
		return Err(format!(
			"{} does not keep versions; turn versioning on first",
			space.name
		));
	}
	let versions = versioning::kept(context, space.source_id, space.id, space.versioning)
		.await
		.ok_or_else(|| format!("{} has no versions", space.name))?;
	Ok((space, versions))
}

async fn describe(
	versions: &sd_store::SpaceVersions,
	names: &ActorNames,
	row: sd_store::SnapshotRow,
) -> Result<SpaceSnapshot, String> {
	let files = versions
		.manifest(Some(row.manifest_id), None)
		.await
		.map_err(|e| e.to_string())?;
	Ok(SpaceSnapshot {
		files: files.len() as u64,
		bytes: files.iter().filter_map(|file| file.size).sum::<i64>() as u64,
		created_by: names.get(row.created_by, "app_op"),
		id: row.id,
		name: row.name,
		manifest_id: row.manifest_id,
		at: row.at,
	})
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceSnapshotCreateInput {
	pub id: Uuid,
	pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceSnapshotCreateOutput {
	pub snapshot: SpaceSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceSnapshotCreateAction {
	input: SpaceSnapshotCreateInput,
}

impl LibraryAction for SpaceSnapshotCreateAction {
	type Input = SpaceSnapshotCreateInput;
	type Output = SpaceSnapshotCreateOutput;

	fn from_input(mut input: Self::Input) -> Result<Self, String> {
		input.name = input.name.trim().to_string();
		if input.name.is_empty() {
			return Err("name the snapshot".to_string());
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let (_, versions) = versions_of(&library, &context, self.input.id, true)
			.await
			.map_err(ActionError::InvalidInput)?;
		let person = actors::this_person(&library, &context)
			.await
			.map_err(ActionError::Internal)?;
		let row = versions
			.create_snapshot(&self.input.name, person, &chrono::Utc::now().to_rfc3339())
			.await
			.map_err(|e| ActionError::InvalidInput(e.to_string()))?;
		let names = ActorNames::load(&library, &context)
			.await
			.map_err(ActionError::Internal)?;
		Ok(SpaceSnapshotCreateOutput {
			snapshot: describe(&versions, &names, row)
				.await
				.map_err(ActionError::Internal)?,
		})
	}

	fn action_kind(&self) -> &'static str {
		"spaces.snapshot.create"
	}
}

crate::register_library_action!(SpaceSnapshotCreateAction, "spaces.snapshot.create");

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceSnapshotListInput {
	pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceSnapshotListOutput {
	/// Oldest first. Empty when the space has no versions.
	pub snapshots: Vec<SpaceSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceSnapshotListQuery {
	input: SpaceSnapshotListInput,
}

impl LibraryQuery for SpaceSnapshotListQuery {
	type Input = SpaceSnapshotListInput;
	type Output = SpaceSnapshotListOutput;

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
		if !space.versioning && !versioning::has_versions(&context, space.source_id, space.id) {
			return Ok(SpaceSnapshotListOutput {
				snapshots: Vec::new(),
			});
		}
		let (_, versions) = versions_of(&library, &context, space.id, false)
			.await
			.map_err(QueryError::Internal)?;
		let names = ActorNames::load(&library, &context)
			.await
			.map_err(QueryError::Internal)?;
		let mut snapshots = Vec::new();
		for row in versions
			.snapshots()
			.await
			.map_err(|e| QueryError::Internal(e.to_string()))?
		{
			snapshots.push(
				describe(&versions, &names, row)
					.await
					.map_err(QueryError::Internal)?,
			);
		}
		Ok(SpaceSnapshotListOutput { snapshots })
	}
}

crate::register_library_query!(SpaceSnapshotListQuery, "spaces.snapshot.list");

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceSnapshotDeleteInput {
	pub id: Uuid,
	pub snapshot: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceSnapshotDeleteOutput {
	pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceSnapshotDeleteAction {
	input: SpaceSnapshotDeleteInput,
}

impl LibraryAction for SpaceSnapshotDeleteAction {
	type Input = SpaceSnapshotDeleteInput;
	type Output = SpaceSnapshotDeleteOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let (_, versions) = versions_of(&library, &context, self.input.id, false)
			.await
			.map_err(ActionError::InvalidInput)?;
		let deleted = versions
			.delete_snapshot(self.input.snapshot)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;
		Ok(SpaceSnapshotDeleteOutput { deleted })
	}

	fn action_kind(&self) -> &'static str {
		"spaces.snapshot.delete"
	}
}

crate::register_library_action!(SpaceSnapshotDeleteAction, "spaces.snapshot.delete");
