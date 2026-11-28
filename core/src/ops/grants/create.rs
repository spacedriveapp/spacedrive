//! Mint a grant over a directory inside a tracked source.
//!
//! The directory has to sit inside a source in this library: a grant is a
//! view of a source, and removing the source ends every grant on it. A
//! filesystem path is canonicalized here, so the stored subtree is the real
//! directory and not whichever link or alias the caller typed. A bucket
//! prefix, `s3://bucket/prefix`, is stored in its canonical URL spelling.
//!
//! A task grant lasts 5 minutes to 12 hours. A standing grant lasts up to 30
//! days and is only for a workspace space: an agent's own folder, which it
//! can hold a credential for across tasks without reaching anything the
//! person keeps elsewhere. Its token is signed by default, so a replica of the
//! space can check it.

use std::{
	path::{Path, PathBuf},
	sync::Arc,
};

use chrono::{DateTime, Duration, Utc};
use sea_orm::{ActiveValue::Set, EntityTrait};
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::info;
use uuid::Uuid;

use super::{
	bucket::{self, BucketUrl},
	scope, signed, store, token, ttl_for, GrantCap, GrantInfo, GrantKind, TokenFormat,
};
use sd_store::{GrantIssuer, GrantRow};

use crate::{
	context::CoreContext,
	domain::SpaceKind,
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
	/// Lifetime in seconds. A task grant lives from 5 minutes to 12 hours,
	/// 1 hour by default; a standing grant up to 30 days, 30 days by default.
	#[serde(default)]
	pub ttl_seconds: Option<u32>,
	/// Defaults to a task grant. A standing grant must be on a workspace
	/// space.
	#[serde(default)]
	pub kind: Option<GrantKind>,
	/// Defaults to signed for a standing grant and opaque for a task grant.
	/// A signed token needs the folder to be inside a space.
	#[serde(default)]
	pub token_format: Option<TokenFormat>,
	/// A name for the owner's list, such as the agent it was made for.
	#[serde(default)]
	pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateGrantOutput {
	pub grant: GrantInfo,
	/// The bearer token, `sdg_` for an opaque token or `sdg_s.` for a signed
	/// one. This is the only time it is shown; Spacedrive keeps only its hash.
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
		ttl_for(input.kind.unwrap_or(GrantKind::Task), input.ttl_seconds)?;
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
		let kind = self.input.kind.unwrap_or(GrantKind::Task);
		let lifetime = ttl_for(kind, self.input.ttl_seconds).map_err(ActionError::InvalidInput)?;
		let token_format = self.input.token_format.unwrap_or(match kind {
			GrantKind::Task => TokenFormat::Opaque,
			GrantKind::Standing => TokenFormat::Signed,
		});
		let target = target(&context, library.id(), &self.input.path).await?;
		let minted = insert(
			&context,
			&library,
			Mint {
				target,
				caps: self.input.caps.unwrap_or(GrantCap::Read),
				lifetime,
				kind,
				token_format,
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
	pub kind: GrantKind,
	pub token_format: TokenFormat,
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
///
/// A grant inside a space is also written to the space's store as an
/// assertion, so every node holding the store knows of it. A signed grant
/// needs that: its issuer's key is published there, and a verifier finds it
/// nowhere else.
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
	let device = context
		.device_manager
		.device_id()
		.map_err(|e| ActionError::Internal(format!("no device identity: {e}")))?;
	let space =
		crate::ops::spaces::events::space_of(context, &mint.target.subtree, Some(library.id()))
			.await;
	let space_row = match &space {
		Some(at) => space_store(context, at.source_id)
			.await?
			.db()
			.space(at.space_id)
			.await
			.map_err(|e| ActionError::Internal(format!("read space: {e}")))?,
		None => None,
	};
	if mint.kind == GrantKind::Standing
		&& space_row
			.as_ref()
			.is_none_or(|row| SpaceKind::parse(&row.kind) != SpaceKind::Workspace)
	{
		return Err(ActionError::InvalidInput(format!(
			"a standing grant is only for a workspace space, and {} is not inside one",
			mint.target.subtree.display()
		)));
	}
	if mint.token_format == TokenFormat::Signed && space.is_none() {
		return Err(ActionError::InvalidInput(format!(
			"a signed token is checked against a space's store, and {} is not inside a space",
			mint.target.subtree.display()
		)));
	}

	let id = Uuid::now_v7();
	// A signed token states its expiry in whole seconds, so the row keeps
	// whole seconds too and the lifetime stays exact.
	let now = DateTime::from_timestamp(Utc::now().timestamp(), 0)
		.ok_or_else(|| ActionError::Internal("clock out of range".to_string()))?;
	let expires_at = now + mint.lifetime;
	let epoch = space_row.as_ref().map_or(0, |row| row.authority_epoch);
	let (token, token_hash, signer) = match (mint.token_format, &space) {
		(TokenFormat::Signed, Some(at)) => {
			let master = context
				.device_manager
				.master_key()
				.await
				.map_err(|e| ActionError::Internal(format!("no device key: {e}")))?;
			let key = signed::signing_key(&master);
			let claims = signed::Claims {
				v: signed::CLAIMS_VERSION,
				grant_id: id,
				space_id: at.space_id,
				subtree: at.path.clone(),
				caps: mint.caps,
				exp: expires_at.timestamp(),
				epoch,
				iss: device,
			};
			let token = signed::sign(&key, &claims).ok_or_else(|| {
				ActionError::InvalidInput("the folder's path is too long for a signed token".into())
			})?;
			let hash = token::digest(&token);
			(token, hash, Some(key.verifying_key()))
		}
		_ => {
			let (token, hash) = token::mint();
			(token, hash, None)
		}
	};
	let refresh = mint.oauth_client.as_ref().map(|_| token::mint_refresh());
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

	if let Some(at) = &space {
		let store = space_store(context, at.source_id).await?;
		let stamp = crate::ops::spaces::store::stamp(context).map_err(ActionError::Internal)?;
		let failed = |e: sd_store::Error| ActionError::Internal(format!("record grant: {e}"));
		if let Some(key) = signer {
			store
				.db()
				.put_grant_issuer(&GrantIssuer {
					device_uuid: device,
					public_key: key.to_bytes().to_vec(),
					updated_hlc: stamp.hlc.clone(),
					origin_device: stamp.device_uuid,
				})
				.await
				.map_err(failed)?;
		}
		store
			.db()
			.put_grant(&GrantRow {
				uuid: id,
				space_uuid: at.space_id,
				actor_uuid: Some(actor),
				kind: mint.kind.as_str().to_string(),
				token_format: mint.token_format.as_str().to_string(),
				subtree: at.path.clone(),
				caps: mint.caps.as_str().to_string(),
				label: mint.label.clone(),
				issuer: device,
				epoch,
				created_at: now.to_rfc3339(),
				expires_at: expires_at.to_rfc3339(),
				updated_hlc: stamp.hlc,
				origin_device: stamp.device_uuid,
			})
			.await
			.map_err(failed)?;
	}

	agent_grant::Entity::insert(agent_grant::ActiveModel {
		uuid: Set(id),
		token_hash: Set(token_hash),
		source_id: Set(mint.target.source_id),
		subtree: Set(mint.target.subtree.to_string_lossy().into_owned()),
		caps: Set(mint.caps.as_str().to_string()),
		label: Set(mint.label),
		created_by: Set(device.to_string()),
		created_at: Set(now),
		expires_at: Set(expires_at),
		revoked_at: Set(None),
		oauth_client: Set(mint.oauth_client),
		refresh_hash: Set(refresh.as_ref().map(|(_, hash)| hash.clone())),
		space_id: Set(space.as_ref().map(|at| at.space_id)),
		actor_id: Set(Some(actor)),
		kind: Set(mint.kind.as_str().to_string()),
		token_format: Set(mint.token_format.as_str().to_string()),
		epoch: Set(epoch),
		..Default::default()
	})
	.exec(db)
	.await
	.map_err(|e| ActionError::Internal(format!("store grant: {e}")))?;

	info!(
		grant = %id,
		subtree = %mint.target.subtree.display(),
		kind = mint.kind.as_str(),
		token_format = mint.token_format.as_str(),
		"grant created"
	);
	let grant = store::info_by_id(db, id)
		.await
		.map_err(|e| ActionError::Internal(e.to_string()))?;
	Ok(Minted {
		grant,
		token,
		refresh: refresh.map(|(refresh, _)| refresh),
	})
}

/// The writable store of the source that holds a space.
pub(crate) async fn space_store(
	context: &CoreContext,
	source_id: Uuid,
) -> Result<Arc<crate::ops::indexing::store::SourceStore>, ActionError> {
	let index = context.volume_index();
	let root = index.source_root(source_id).ok_or_else(|| {
		ActionError::InvalidInput("the source holding this space is not on this device".into())
	})?;
	index.store_for(&root).await.ok_or_else(|| {
		ActionError::Internal("the source holding this space has no store on this device".into())
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
		let roots = roots.into_iter().filter_map(|(id, registered)| {
			let canonical = std::fs::canonicalize(&registered).ok()?;
			Some((id, registered, canonical))
		});
		innermost_holding(roots, &subtree, |a, b| {
			same_file::is_same_file(a, b).unwrap_or(false)
		})
	})
	.await
	.map_err(|e| e.to_string())
}

/// The innermost of `roots`, each an id with its registered and canonical
/// spelling, that holds `subtree`, with `subtree` respelled under the
/// registered root. `same` decides directory identity, as in
/// [`scope::within`].
fn innermost_holding(
	roots: impl IntoIterator<Item = (Uuid, PathBuf, PathBuf)>,
	subtree: &Path,
	same: impl Fn(&Path, &Path) -> bool,
) -> Option<(Uuid, PathBuf)> {
	roots
		.into_iter()
		.filter_map(|(id, registered, canonical)| {
			let rest = scope::within(&canonical, subtree, &same)?;
			let path = if rest.as_os_str().is_empty() {
				registered
			} else {
				registered.join(&rest)
			};
			Some((rest.components().count(), id, path))
		})
		.min_by_key(|(depth, _, _)| *depth)
		.map(|(_, id, path)| (id, path))
}

crate::register_library_action!(CreateGrantAction, "grants.create");

#[cfg(test)]
mod tests {
	use super::*;

	/// Stands in for a macOS firmlink on any platform: a path and its
	/// /System/Volumes/Data spelling are one directory.
	fn firmlinked(a: &Path, b: &Path) -> bool {
		let plain = |path: &Path| {
			path.strip_prefix("/System/Volumes/Data")
				.map(|rest| Path::new("/").join(rest))
				.unwrap_or_else(|_| path.to_path_buf())
		};
		plain(a) == plain(b)
	}

	#[test]
	fn a_grant_finds_its_source_under_either_spelling() {
		let (outer, inner) = (Uuid::now_v7(), Uuid::now_v7());
		let roots = || {
			[
				(outer, "/System/Volumes/Data/Users/me"),
				(inner, "/System/Volumes/Data/Users/me/work"),
			]
			.map(|(id, root)| (id, PathBuf::from(root), PathBuf::from(root)))
		};
		for subtree in [
			"/Users/me/work/project",
			"/System/Volumes/Data/Users/me/work/project",
		] {
			assert_eq!(
				innermost_holding(roots(), Path::new(subtree), firmlinked),
				Some((
					inner,
					PathBuf::from("/System/Volumes/Data/Users/me/work/project")
				)),
				"{subtree}"
			);
		}
		assert_eq!(
			innermost_holding(roots(), Path::new("/Users/someone"), firmlinked),
			None
		);
	}
}
