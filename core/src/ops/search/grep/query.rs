//! The `search.grep` query: resolve the scope on this device, then scan it.

use std::{
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};

use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::debug;
use uuid::Uuid;

use super::{
	scan::{self, GrepError},
	GrepInput, GrepOutput,
};
use crate::{
	context::CoreContext,
	domain::addressing::SdPath,
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::sources::registry,
};

/// Search file contents under a path, live, on the device that holds them.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GrepQuery {
	pub input: GrepInput,
}

impl LibraryQuery for GrepQuery {
	type Input = GrepInput;
	type Output = GrepOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		input
			.options
			.validate()
			.map_err(|message| QueryError::Validation {
				field: "options".to_string(),
				message,
			})?;
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let LocalScope { root, relative, .. } =
			resolve_local_scope(&context, &session, &self.input.scope, "grep").await?;

		debug!(scope = %root.join(&relative).display(), pattern = %self.input.options.pattern, "search.grep");

		// Dropping this future, as a disconnected client does, stops the scan.
		let cancel = Arc::new(AtomicBool::new(false));
		let _stop = CancelOnDrop(cancel.clone());
		let options = self.input.options;
		tokio::task::spawn_blocking(move || scan::grep(&root, &relative, &options, &cancel))
			.await
			.map_err(|e| QueryError::Internal(format!("grep task failed: {e}")))?
			.map_err(|error| match error {
				GrepError::Cancelled => QueryError::Cancelled,
				GrepError::Scope { path, source } => QueryError::Io { path, source },
				other => QueryError::InvalidInput(other.to_string()),
			})
	}
}

/// A path inside a source of the session's library, tracked on this device.
pub(crate) struct LocalScope {
	pub source_id: Uuid,
	/// The source root as registered, which is how its store is found.
	pub registered_root: PathBuf,
	/// The same root with symlinks resolved.
	pub root: PathBuf,
	/// The requested path below `root`, symlinks resolved.
	pub relative: PathBuf,
}

/// Resolve a requested path to the tracked source that holds it.
///
/// The path must sit inside a source of the session's library and stay
/// inside it once symlinks in it are resolved. `what` names the operation
/// in the errors a caller sees.
pub(crate) async fn resolve_local_scope(
	context: &Arc<CoreContext>,
	session: &crate::infra::api::SessionContext,
	scope: &SdPath,
	what: &str,
) -> QueryResult<LocalScope> {
	let requested = match scope {
		SdPath::Physical { device_slug, path } => {
			if !SdPath::same_device(device_slug, "local") {
				return Err(QueryError::InvalidInput(format!(
					"{what} over a source on device '{device_slug}' is not available on this device yet; run it on that device"
				)));
			}
			path.clone()
		}
		_ => {
			return Err(QueryError::InvalidInput(format!(
				"{what} needs a filesystem path on this device"
			)))
		}
	};

	let library_id = session
		.current_library_id
		.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;
	let library = context
		.libraries()
		.await
		.get_library(library_id)
		.await
		.ok_or(QueryError::LibraryNotFound(library_id))?;

	let canonical = tokio::fs::canonicalize(&requested)
		.await
		.map_err(|source| QueryError::Io {
			path: requested.display().to_string(),
			source,
		})?;

	let index = context.volume_index();
	let not_tracked = || {
		QueryError::InvalidInput(format!(
			"{} is not under a source tracked on this device",
			requested.display()
		))
	};
	let source_id = index
		.source_id_for(&requested)
		.or_else(|| index.source_id_for(&canonical))
		.ok_or_else(not_tracked)?;
	registry::get(library.db().conn(), source_id)
		.await
		.map_err(|_| not_tracked())?;
	let registered_root = index.source_root(source_id).ok_or_else(not_tracked)?;
	let root = tokio::fs::canonicalize(&registered_root)
		.await
		.unwrap_or_else(|_| registered_root.clone());
	let Ok(relative) = canonical.strip_prefix(&root).map(Path::to_path_buf) else {
		return Err(QueryError::InvalidInput(format!(
			"{} resolves outside its source",
			requested.display()
		)));
	};
	Ok(LocalScope {
		source_id,
		registered_root,
		root,
		relative,
	})
}

struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
	fn drop(&mut self) {
		self.0.store(true, Ordering::Relaxed);
	}
}

crate::register_library_query!(GrepQuery, "search.grep");
