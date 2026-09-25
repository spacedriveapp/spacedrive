//! A path on this machine that another app can open a file at.
//!
//! A file on this device opens at its own path. A file on another device opens
//! at its path inside the mounts share mounted on this machine, which streams
//! whatever the app reads through the block cache, so an app can play a video
//! larger than this machine's free disk. Handing an opener the other device's
//! path would open a different file here, or none.
//!
//! This is a query although the first call may mount the share. The mount
//! changes nothing in the library, and viewers ask for paths as fast as a
//! cursor moves.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::context::CoreContext;
use crate::domain::SdPath;
use crate::infra::query::{CoreQuery, QueryError, QueryResult};
use crate::service::mounts;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocalPathInput {
	pub path: SdPath,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocalPathOutput {
	pub path: PathBuf,
}

pub struct LocalPathQuery {
	input: LocalPathInput,
}

impl CoreQuery for LocalPathQuery {
	type Input = LocalPathInput;
	type Output = LocalPathOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let path = &self.input.path;
		if let Some(local) = path.as_local_path() {
			return Ok(LocalPathOutput {
				path: local.to_path_buf(),
			});
		}
		if !matches!(path, SdPath::Physical { .. }) {
			return Err(QueryError::InvalidInput(format!(
				"{path} has no path on this machine"
			)));
		}
		let share_path = mounts::share_path(&context, path).await.ok_or_else(|| {
			QueryError::InvalidInput(format!("{path} is not in a source this device replicates"))
		})?;
		let mount_point = mounts::attach::attach(&context)
			.await
			.map_err(|error| QueryError::Internal(error.to_string()))?;
		Ok(LocalPathOutput {
			path: mount_point.join(share_path),
		})
	}
}

crate::register_core_query!(LocalPathQuery, "files.local_path");
