//! Where Spacedrive's own viewers stream a file from.
//!
//! The mounts share serves every source on this device and every source it
//! replicates from a paired device over loopback HTTP, with ranges and content
//! types, and reads another device's bytes through the block cache. A viewer
//! that plays a video from it reads only the ranges it plays.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::context::CoreContext;
use crate::domain::SdPath;
use crate::infra::query::{CoreQuery, QueryError, QueryResult};
use crate::service::mounts;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct StreamUrlInput {
	pub path: SdPath,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct StreamUrlOutput {
	/// A loopback URL that answers ranged `GET`s for the file's bytes.
	pub url: String,
}

pub struct StreamUrlQuery {
	input: StreamUrlInput,
}

impl CoreQuery for StreamUrlQuery {
	type Input = StreamUrlInput;
	type Output = StreamUrlOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let share_path = mounts::share_path(&context, &self.input.path)
			.await
			.ok_or_else(|| {
				QueryError::InvalidInput(format!(
					"{} is not in a source this device serves",
					self.input.path
				))
			})?;
		let url = mounts::file_url(&share_path)
			.ok_or_else(|| QueryError::Internal("the mounts share is not running".to_string()))?;
		Ok(StreamUrlOutput { url })
	}
}

crate::register_core_query!(StreamUrlQuery, "files.stream_url");
