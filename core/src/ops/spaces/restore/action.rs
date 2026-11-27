//! `spaces.restore`: write an earlier version of a file, or a snapshot of
//! a folder, back into a space.
//!
//! A restore goes through the write path like any other change: it is
//! refused if the file changes under it, the bytes it replaces go to the
//! trash, it appears in the space's changes as a restore by the person, and
//! `files.undo` reverses it.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::{
		actors,
		spaces::{
			events::{Attribution, Origin},
			store,
			versioning::{self, RestoreFrom},
		},
	},
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceRestoreInput {
	pub id: Uuid,
	/// One version, by the id `spaces.history` gave it.
	#[serde(default)]
	pub version: Option<i64>,
	/// A snapshot, by id or name.
	#[serde(default)]
	pub snapshot: Option<String>,
	/// With `version`, where to write it, its own path by default. With
	/// `snapshot`, the file or folder to restore, the whole space by
	/// default. Relative to the space root, `/`-separated.
	#[serde(default)]
	pub path: Option<String>,
}

/// One file a restore wrote.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RestoredFile {
	/// Relative to the space root.
	pub path: String,
	/// Where it landed: the path, or a conflict name beside it when
	/// something took the name during the restore.
	pub landed: String,
	pub conflict: bool,
	/// Pass to `files.undo` to reverse it.
	pub undo: Option<Uuid>,
}

/// One file a restore could not write.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RestoreFailure {
	pub path: String,
	pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceRestoreOutput {
	pub restored: Vec<RestoredFile>,
	/// Files a snapshot restore left because they already match.
	pub unchanged: u32,
	pub failed: Vec<RestoreFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceRestoreAction {
	input: SpaceRestoreInput,
}

impl LibraryAction for SpaceRestoreAction {
	type Input = SpaceRestoreInput;
	type Output = SpaceRestoreOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		if input.version.is_some() == input.snapshot.is_some() {
			return Err("name a version or a snapshot to restore".to_string());
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let space = store::find(&library, &context, self.input.id)
			.await
			.map_err(ActionError::InvalidInput)?;
		if !space.versioning {
			return Err(ActionError::InvalidInput(format!(
				"{} does not keep versions; turn versioning on first",
				space.name
			)));
		}
		let versions = versioning::open(&context, space.source_id, space.id)
			.await
			.ok_or_else(|| {
				ActionError::Internal("the space's versions are not on this device".to_string())
			})?;
		let from = match (self.input.version, self.input.snapshot) {
			(Some(version), _) => RestoreFrom::Version(version),
			(None, Some(snapshot)) => RestoreFrom::Snapshot(snapshot),
			(None, None) => unreachable!("checked in from_input"),
		};
		let plan = versioning::plan(&versions, &from, self.input.path.as_deref())
			.await
			.map_err(ActionError::InvalidInput)?;
		let person = actors::this_person(&library, &context)
			.await
			.map_err(ActionError::Internal)?;
		let who = Attribution {
			origin: Origin::Restore,
			actor: person,
			grant: None,
			undo: None,
			library: Some(library.id()),
		};
		let mut output = SpaceRestoreOutput {
			restored: Vec::new(),
			unchanged: 0,
			failed: Vec::new(),
		};
		for (path, hash) in plan {
			let written = match versioning::person_scope(library.id(), &space, person, &path).await
			{
				Ok(scope) => {
					versioning::restore_file(&context, versions.clone(), &scope, who, hash)
						.await
						.map_err(|e| e.to_string())
				}
				Err(error) => Err(error),
			};
			match written {
				Ok(None) => output.unchanged += 1,
				Ok(Some(outcome)) => output.restored.push(RestoredFile {
					path,
					landed: outcome.state.path,
					conflict: outcome.conflict,
					undo: outcome.undo,
				}),
				Err(error) => output.failed.push(RestoreFailure { path, error }),
			}
		}
		if output.restored.is_empty() && !output.failed.is_empty() {
			let failure = &output.failed[0];
			return Err(ActionError::InvalidInput(format!(
				"could not restore {}: {}",
				failure.path, failure.error
			)));
		}
		Ok(output)
	}

	fn action_kind(&self) -> &'static str {
		"spaces.restore"
	}
}

crate::register_library_action!(SpaceRestoreAction, "spaces.restore");
