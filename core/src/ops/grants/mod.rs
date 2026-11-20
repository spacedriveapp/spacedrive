//! # Agent grants
//!
//! `core::ops::grants` lends one directory of one source to an agent for a
//! bounded time. A grant is a row in `library.db` and an opaque bearer token
//! the owner hands to the agent. The token is shown once, at creation, and
//! only its SHA-256 is stored, so neither a backup nor a stolen database
//! yields a working credential.
//!
//! Every agent-facing surface checks a request through one function,
//! [`authorize`]. It reads the grant row on every call, so a revocation or an
//! expiry refuses the very next request, and it resolves the requested path
//! inside the granted directory by filesystem identity rather than by string
//! prefix. That makes `..`, a symlink pointing out of the directory, a macOS
//! firmlink spelling, and a case-only alias on a case-insensitive volume all
//! land on the directory they really name. Writable grants additionally refuse
//! instruction files and git internals, see [`scope::protected_component`].
//!
//! Every allowed or refused request against a known grant is written to a
//! bounded audit table, [`audit`]. Removing a source revokes every grant on
//! it, so re-adding the same scope never revives an old token.
//!
//! The local RPC socket does not go through grants: it stays as trusted as it
//! is today. `docs/core/design/agent-grants.md` has the full model.
//!
//! ## Example
//! ```rust,no_run
//! # async fn demo(context: &sd_core::context::CoreContext) {
//! use sd_core::ops::grants::{authorize, GrantCap};
//! use std::path::Path;
//!
//! let token = "sdg_...";
//! match authorize(context, token, "read", Path::new("notes/today.md"), GrantCap::Read).await {
//! 	Ok(scope) => tracing::debug!(path = %scope.path.display(), "serve"),
//! 	Err(refusal) => tracing::warn!(%refusal, "refused"),
//! }
//! # }
//! ```

pub mod audit;
mod authorize;
pub mod create;
pub mod extend;
pub mod list;
pub mod revoke;
pub mod scope;
pub mod store;
pub mod token;

use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

pub use authorize::{authorize, authorize_in};

/// The shortest lifetime a grant can be created or extended with.
pub const MIN_TTL_SECONDS: u32 = 5 * 60;
/// The longest lifetime, measured from the moment of creation or extension.
pub const MAX_TTL_SECONDS: u32 = 12 * 60 * 60;
/// The lifetime a grant gets when the caller names none.
pub const DEFAULT_TTL_SECONDS: u32 = 60 * 60;

/// The lifetime a caller asked for, checked against the bounds.
pub(crate) fn ttl(seconds: Option<u32>) -> Result<Duration, String> {
	let seconds = seconds.unwrap_or(DEFAULT_TTL_SECONDS);
	if !(MIN_TTL_SECONDS..=MAX_TTL_SECONDS).contains(&seconds) {
		return Err(format!(
			"a grant lives between {} minutes and {} hours, not {seconds} seconds",
			MIN_TTL_SECONDS / 60,
			MAX_TTL_SECONDS / 3600
		));
	}
	Ok(Duration::seconds(seconds.into()))
}

/// What a grant lets its holder do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GrantCap {
	Read,
	ReadWrite,
}

impl GrantCap {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Read => "read",
			Self::ReadWrite => "read_write",
		}
	}

	pub fn parse(value: &str) -> Option<Self> {
		match value {
			"read" => Some(Self::Read),
			"read_write" => Some(Self::ReadWrite),
			_ => None,
		}
	}

	/// Whether a grant holding `self` may perform a request needing `wanted`.
	pub fn allows(self, wanted: GrantCap) -> bool {
		self == Self::ReadWrite || wanted == Self::Read
	}
}

/// Whether a grant can still be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GrantStatus {
	Active,
	Expired,
	Revoked,
}

/// A grant as the owner sees it. Never carries the token.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GrantInfo {
	pub id: Uuid,
	pub source_id: Uuid,
	/// The granted directory, canonical at creation.
	pub subtree: PathBuf,
	pub caps: GrantCap,
	pub label: Option<String>,
	/// The device that minted the grant.
	pub created_by: String,
	pub created_at: DateTime<Utc>,
	pub expires_at: DateTime<Utc>,
	pub revoked_at: Option<DateTime<Utc>>,
	pub status: GrantStatus,
	/// When the grant last authorized a request, from the audit log.
	pub last_used_at: Option<DateTime<Utc>>,
}

/// What an authorized request may touch.
///
/// `path` is the one the surface should open. It is spelled under `root`
/// whatever spelling the agent sent, so a firmlink or case alias never
/// reaches the filesystem call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantScope {
	pub grant_id: Uuid,
	pub library_id: Uuid,
	pub cap: GrantCap,
	/// The granted directory, canonical now.
	pub root: PathBuf,
	/// The requested path, absolute and resolved inside `root`.
	pub path: PathBuf,
	/// `path` relative to `root`. Empty for the root itself.
	pub relative: PathBuf,
	pub expires_at: DateTime<Utc>,
}

/// Why a request was refused.
///
/// The first four mean the token is not usable at all, which an HTTP surface
/// answers with 401. The rest mean this one request is outside the grant,
/// which it answers with 403. [`GrantError::is_credential`] tells them apart.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GrantError {
	#[error("unknown grant token")]
	UnknownToken,
	#[error("the grant has expired")]
	Expired,
	#[error("the grant was revoked")]
	Revoked,
	#[error("the granted source is no longer in the library")]
	SourceGone,
	#[error("the grant is read-only")]
	ReadOnly,
	#[error("the path is outside the granted directory: {0}")]
	Escape(String),
	#[error("writable grants cannot modify {0}")]
	Protected(String),
	#[error("the granted directory is unavailable: {0}")]
	RootUnavailable(String),
	#[error("invalid path: {0}")]
	InvalidPath(String),
	#[error("internal error: {0}")]
	Internal(String),
}

impl GrantError {
	/// Whether the token itself is unusable, as opposed to this one request.
	pub fn is_credential(&self) -> bool {
		matches!(
			self,
			Self::UnknownToken | Self::Expired | Self::Revoked | Self::SourceGone
		)
	}
}
