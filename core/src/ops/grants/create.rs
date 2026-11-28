//! Mint a grant over a directory inside a tracked source.
//!
//! The directory has to sit inside a source in this library: a grant is a
//! view of a source, and removing the source ends every grant on it. A
//! filesystem path is canonicalized here, so the stored subtree is the real
//! directory and not whichever link or alias the caller typed. A bucket
//! prefix, `s3://bucket/prefix`, is stored in its canonical URL spelling.

use std::{
	path::{Path, PathBuf},
	sync::Arc,
};

use chrono::{Duration, Utc};
use sea_orm::{ActiveValue::Set, EntityTrait};
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::info;
use uuid::Uuid;

use super::{
	bucket::{self, BucketUrl},
	scope, store, token, ttl, GrantCap, GrantInfo,
};
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
	/// The directory to grant, on this device, or a bucket prefix such as
	/// `s3://bucket/prefix` inside a tracked bucket source.
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
		let target = target(&context, library.id(), &self.input.path).await?;
		let minted = insert(
			&context,
			&library,
			Mint {
				target,
				caps: self.input.caps.unwrap_or(GrantCap::Read),
				lifetime,
				label: self.input.label.clone(),
				oauth_client: None,
			},
		)
		.await?;
		Ok(CreateGrantOutput {
			grant: minted.grant,
			token: minted.token,
		})
	}

	fn action_kind(&self) -> &'static str {
		"grants.create"
	}
}

/// A directory to grant and the source it belongs to, both checked.
#[derive(Debug, Clone)]
pub(crate) struct Target {
	pub subtree: PathBuf,
	pub source_id: Uuid,
}

/// Everything a new grant row needs.
pub(crate) struct Mint {
	pub target: Target,
	pub caps: GrantCap,
	pub lifetime: Duration,
	pub label: Option<String>,
	/// Set for a grant issued through OAuth, which also gets a refresh token.
	pub oauth_client: Option<String>,
}

/// A new grant and its secrets, each shown only here.
pub(crate) struct Minted {
	pub grant: GrantInfo,
	pub token: String,
	pub refresh: Option<String>,
}

/// Canonicalize `path` and find the source of `library` that holds it.
pub(crate) async fn target(
	context: &CoreContext,
	library: Uuid,
	path: &Path,
) -> Result<Target, ActionError> {
	if let Some(url) = BucketUrl::parse(path) {
		return bucket_target(context, library, url);
	}
	let requested = path.to_path_buf();
	let subtree = tokio::task::spawn_blocking(move || directory(&requested))
		.await
		.map_err(|e| ActionError::Internal(e.to_string()))??;
	let source_id = owning_source(context, library, &subtree)
		.await
		.map_err(ActionError::Internal)?
		.map(|(id, _)| id)
		.ok_or_else(|| {
			ActionError::InvalidInput(format!(
				"{} is not inside a source of this library; track it first with `sd sources track`",
				subtree.display()
			))
		})?;
	Ok(Target { subtree, source_id })
}

/// A prefix of a bucket source of `library`, the innermost one holding it.
///
/// A prefix needs no object under it: an agent with a writable grant may
/// start an empty one.
fn bucket_target(
	context: &CoreContext,
	library: Uuid,
	url: BucketUrl,
) -> Result<Target, ActionError> {
	let source_id = context
		.volume_index()
		.sources_of(library)
		.into_iter()
		.filter_map(|source| {
			let root = BucketUrl::parse(&source.root)?;
			root.contains(&url).then(|| (root.key.len(), source.id))
		})
		.max_by_key(|(depth, _)| *depth)
		.map(|(_, id)| id)
		.ok_or_else(|| {
			ActionError::InvalidInput(format!(
				"{} is not inside a bucket source of this library; track it first with `sd sources track {}`",
				url.path().display(),
				url.volume()
			))
		})?;
	Ok(Target {
		subtree: url.path(),
		source_id,
	})
}

/// Write a grant row. The owner's action and the OAuth consent page both
/// mint through here, so a grant is the same thing whichever way it came.
pub(crate) async fn insert(
	context: &CoreContext,
	library: &Library,
	mint: Mint,
) -> Result<Minted, ActionError> {
	if let Some(url) = BucketUrl::parse(&mint.target.subtree) {
		if mint.caps == GrantCap::ReadWrite {
			let backend = bucket::backend(&context.volume_manager, &url)
				.await
				.map_err(|e| ActionError::InvalidInput(e.to_string()))?;
			bucket::writable(&url, backend.as_ref())
				.map_err(|e| ActionError::InvalidInput(e.to_string()))?;
		}
	}
	let db = library.db().conn();
	let created_by = context
		.device_manager
		.device_id()
		.map_err(|e| ActionError::Internal(format!("no device identity: {e}")))?
		.to_string();
	let (token, token_hash) = token::mint();
	let refresh = mint.oauth_client.as_ref().map(|_| token::mint_refresh());
	let id = Uuid::now_v7();
	let now = Utc::now();
	// Every grant is lent to its own new agent, so that agent's history is
	// one actor whichever paths it touches.
	let actor = crate::ops::actors::new_agent(
		library,
		context,
		mint.label.clone().unwrap_or_else(|| "Agent".to_string()),
		mint.oauth_client.clone(),
	)
	.await
	.map_err(ActionError::Internal)?;
	let space =
		crate::ops::spaces::events::space_of(context, &mint.target.subtree, Some(library.id()))
			.await
			.map(|at| at.space_id);
	agent_grant::Entity::insert(agent_grant::ActiveModel {
		uuid: Set(id),
		token_hash: Set(token_hash),
		source_id: Set(mint.target.source_id),
		subtree: Set(mint.target.subtree.to_string_lossy().into_owned()),
		caps: Set(mint.caps.as_str().to_string()),
		label: Set(mint.label),
		created_by: Set(created_by),
		created_at: Set(now),
		expires_at: Set(now + mint.lifetime),
		revoked_at: Set(None),
		oauth_client: Set(mint.oauth_client),
		refresh_hash: Set(refresh.as_ref().map(|(_, hash)| hash.clone())),
		space_id: Set(space),
		actor_id: Set(Some(actor)),
		..Default::default()
	})
	.exec(db)
	.await
	.map_err(|e| ActionError::Internal(format!("store grant: {e}")))?;

	info!(grant = %id, subtree = %mint.target.subtree.display(), "grant created");
	let grant = store::info_by_id(db, id)
		.await
		.map_err(|e| ActionError::Internal(e.to_string()))?;
	Ok(Minted {
		grant,
		token,
		refresh: refresh.map(|(refresh, _)| refresh),
	})
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
