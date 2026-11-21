//! Mint a grant over a directory inside a tracked source.
//!
//! The directory has to sit inside a filesystem source in this library: a
//! grant is a view of a source, and removing the source ends every grant on
//! it. The path is canonicalized here, so the stored subtree is the real
//! directory and not whichever link or alias the caller typed.

use std::{
	path::{Path, PathBuf},
	sync::Arc,
};

use chrono::Utc;
use sea_orm::{ActiveValue::Set, EntityTrait};
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::info;
use uuid::Uuid;

use super::{scope, store, token, ttl, GrantCap, GrantInfo};
use crate::{
	context::CoreContext,
	infra::{
		action::{error::ActionError, LibraryAction},
		db::entities::agent_grant,
	},
	library::Library,
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateGrantInput {
	/// The directory to grant, on this device.
	pub path: PathBuf,
	/// Defaults to read-only.
	#[serde(default)]
	pub caps: Option<GrantCap>,
	/// Lifetime in seconds, from 5 minutes to 12 hours. Defaults to 1 hour.
	#[serde(default)]
	pub ttl_seconds: Option<u32>,
	/// A name for the owner's list, such as the agent it was made for.
	#[serde(default)]
	pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateGrantOutput {
	pub grant: GrantInfo,
	/// The bearer token. This is the only time it is shown; Spacedrive keeps
	/// only its hash.
	pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGrantAction {
	input: CreateGrantInput,
}

impl LibraryAction for CreateGrantAction {
	type Input = CreateGrantInput;
	type Output = CreateGrantOutput;

	fn from_input(input: CreateGrantInput) -> Result<Self, String> {
		ttl(input.ttl_seconds)?;
		if input.label.as_ref().is_some_and(|label| label.len() > 255) {
			return Err("label cannot exceed 255 characters".to_string());
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let lifetime = ttl(self.input.ttl_seconds).map_err(ActionError::InvalidInput)?;
		let db = library.db().conn();

		let requested = self.input.path.clone();
		let subtree = tokio::task::spawn_blocking(move || directory(&requested))
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))??;
		let source_id = owning_source(&context, library.id(), &subtree)
			.await
			.map_err(ActionError::Internal)?
			.map(|(id, _)| id)
			.ok_or_else(|| {
				ActionError::InvalidInput(format!(
					"{} is not inside a source of this library; track it first with `sd sources track`",
					subtree.display()
				))
			})?;

		let created_by = context
			.device_manager
			.device_id()
			.map_err(|e| ActionError::Internal(format!("no device identity: {e}")))?
			.to_string();
		let (token, token_hash) = token::mint();
		let id = Uuid::now_v7();
		let now = Utc::now();
		agent_grant::Entity::insert(agent_grant::ActiveModel {
			uuid: Set(id),
			token_hash: Set(token_hash),
			source_id: Set(source_id),
			subtree: Set(subtree.to_string_lossy().into_owned()),
			caps: Set(self
				.input
				.caps
				.unwrap_or(GrantCap::Read)
				.as_str()
				.to_string()),
			label: Set(self.input.label.clone()),
			created_by: Set(created_by),
			created_at: Set(now),
			expires_at: Set(now + lifetime),
			revoked_at: Set(None),
			..Default::default()
		})
		.exec(db)
		.await
		.map_err(|e| ActionError::Internal(format!("store grant: {e}")))?;

		info!(grant = %id, subtree = %subtree.display(), "grant created");
		let grant = store::info_by_id(db, id)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;
		Ok(CreateGrantOutput { grant, token })
	}

	fn action_kind(&self) -> &'static str {
		"grants.create"
	}
}

/// The canonical spelling of the directory to grant.
fn directory(path: &Path) -> Result<PathBuf, ActionError> {
	let subtree = std::fs::canonicalize(path)
		.map(crate::common::utils::strip_windows_extended_prefix)
		.map_err(|e| ActionError::InvalidInput(format!("{}: {e}", path.display())))?;
	if !subtree.is_dir() {
		return Err(ActionError::InvalidInput(format!(
			"{} is not a directory",
			path.display()
		)));
	}
	Ok(subtree)
}

/// The innermost source of this library holding `subtree`, and `subtree`
/// spelled under that source's registered root.
///
/// Only this library's sources are considered, since another open library
/// may track an enclosing directory, and the volume index is asked because
/// it holds each source's absolute root while the registry row keeps it
/// relative to its volume. Containment is decided as `authorize` decides it,
/// so a canonical path and a registered root that differ by a firmlink still
/// match, and the returned spelling is the one the source's store answers to.
pub(crate) async fn owning_source(
	context: &CoreContext,
	library: Uuid,
	subtree: &Path,
) -> Result<Option<(Uuid, PathBuf)>, String> {
	let roots: Vec<(Uuid, PathBuf)> = context
		.volume_index()
		.sources_of(library)
		.into_iter()
		.map(|source| (source.id, source.root))
		.collect();
	let subtree = subtree.to_path_buf();
	tokio::task::spawn_blocking(move || {
		roots
			.into_iter()
			.filter_map(|(id, registered)| {
				let root = std::fs::canonicalize(&registered).ok()?;
				let rest = scope::within(&root, &subtree, |a, b| {
					same_file::is_same_file(a, b).unwrap_or(false)
				})?;
				let path = if rest.as_os_str().is_empty() {
					registered
				} else {
					registered.join(&rest)
				};
				Some((rest.components().count(), id, path))
			})
			.min_by_key(|(depth, _, _)| *depth)
			.map(|(_, id, path)| (id, path))
	})
	.await
	.map_err(|e| e.to_string())
}

crate::register_library_action!(CreateGrantAction, "grants.create");
