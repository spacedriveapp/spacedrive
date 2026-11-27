//! # Footage summaries
//!
//! `media.footage.summaries` answers what footage work made for many videos
//! at once. Explorer columns and grid badges show a duration, a shot count
//! and a transcript marker per row, so a listing asks once for every video
//! it shows rather than once per row. Media sidecars are keyed by the sampled
//! content hash, which the source store already holds for every identified
//! file, so this reads the store and the sidecar directory and never opens
//! the videos themselves. A video the store has not identified yet, or one
//! on another device, is left out.
//!
//! ## Example
//! ```rust
//! use sd_core::domain::SdPath;
//! use sd_core::infra::query::LibraryQuery;
//! use sd_core::ops::media::footage::{FootageSummariesInput, FootageSummariesQuery};
//!
//! let listing = FootageSummariesInput {
//! 	paths: vec![SdPath::local("/footage/beach-walk.mp4")],
//! };
//! assert!(FootageSummariesQuery::from_input(listing).is_ok());
//!
//! // A bigger listing is sent in batches.
//! let too_many = FootageSummariesInput {
//! 	paths: vec![SdPath::local("/footage/beach-walk.mp4"); 5001],
//! };
//! assert!(FootageSummariesQuery::from_input(too_many).is_err());
//! ```

use std::sync::Arc;

use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use super::status::analyzed_shots;
use crate::{
	context::CoreContext,
	domain::{ContentSidecar, SdPath, SidecarFormat},
	infra::{
		api::SessionContext,
		query::{LibraryQuery, QueryError, QueryResult},
	},
	ops::media::shots::read_shots,
	service::sidecars::SidecarRegistry,
};

/// More rows than any listing renders at once.
const MAX_PATHS: usize = 5000;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FootageSummariesInput {
	/// The videos a listing shows.
	pub paths: Vec<SdPath>,
}

/// The footage state of one video.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct FootageSummary {
	pub path: SdPath,
	pub transcript: bool,
	/// Shots in the shot list, `None` until shot detection has run.
	pub shots: Option<u32>,
	/// Shots the Videos extension has analyzed, `None` until it has run.
	pub analyzed_shots: Option<u32>,
	/// The length shot detection measured, `None` until it has run.
	pub duration_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FootageSummariesOutput {
	/// One entry per video that has any footage sidecar. A path missing
	/// here has had nothing made for it.
	pub summaries: Vec<FootageSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FootageSummariesQuery {
	input: FootageSummariesInput,
}

impl LibraryQuery for FootageSummariesQuery {
	type Input = FootageSummariesInput;
	type Output = FootageSummariesOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		if input.paths.len() > MAX_PATHS {
			return Err(QueryError::InvalidInput(format!(
				"ask for at most {MAX_PATHS} videos at once"
			)));
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or(QueryError::LibraryNotFound(library_id))?;
		let finder = &SidecarRegistry::of(&library).finder();
		let context = &context;

		let results: Vec<QueryResult<Option<FootageSummary>>> = stream::iter(self.input.paths)
			.map(|path| async move {
				let Some(content_uuid) = content_uuid(context, &path).await else {
					return Ok(None);
				};
				let transcript = finder
					.find(&ContentSidecar::transcript(
						content_uuid,
						SidecarFormat::Json,
					))
					.is_some();
				let list = read_shots(finder, content_uuid).await.map_err(|error| {
					QueryError::Internal(format!("could not read the shot list: {error}"))
				})?;
				let analyzed_shots = analyzed_shots(finder, content_uuid).await?;
				if !transcript && list.is_none() && analyzed_shots.is_none() {
					return Ok(None);
				}
				Ok(Some(FootageSummary {
					path,
					transcript,
					shots: list
						.as_ref()
						.map(|list| u32::try_from(list.shots.len()).unwrap_or(u32::MAX)),
					analyzed_shots,
					duration_seconds: list.map(|list| list.duration.seconds),
				}))
			})
			.buffer_unordered(16)
			.collect()
			.await;

		let mut summaries = Vec::new();
		for result in results {
			summaries.extend(result?);
		}
		Ok(FootageSummariesOutput { summaries })
	}
}

/// The key media sidecars use for the file at `path`: the uuid of the
/// sampled hash the source store recorded, the same one hashing the file
/// would give.
async fn content_uuid(context: &CoreContext, path: &SdPath) -> Option<Uuid> {
	let index = context.volume_index();
	for reach in
		crate::ops::paths::reach::stores_beneath_in(&context.volume_manager, index, path).await
	{
		if !reach.prefix.is_empty() {
			continue;
		}
		let db = index.read_store(reach.source.id).await?;
		if let Ok(Some(entry)) = sd_store::read::entry_by_path(db.pool(), &reach.scope).await {
			return entry
				.sampled_hash
				.as_deref()
				.map(sd_store::content::uuid_for);
		}
	}
	None
}

crate::register_library_query!(FootageSummariesQuery, "media.footage.summaries");
