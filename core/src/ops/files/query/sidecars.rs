//! # Sidecars of one file
//!
//! `files.sidecars` lists everything this device has derived from one file,
//! whichever store holds it: the content sidecars the registry lists across
//! the library's sources (proxies, speech audio, transcripts, shot lists,
//! keyframes and the documents extensions write), and the thumbnail tile and
//! video scrub sheet the thumbnail service keeps by record. Nothing is
//! generated to answer it.
//!
//! Content sidecars are keyed by the candidate content uuid, which comes from
//! sampling the file, so a viewer asks for the one file it shows rather than
//! a listing carrying it on every row. A file on another device answers
//! `null`, since sidecars are made and kept where the bytes are.
//!
//! ## Example
//! ```json
//! { "path": { "Physical": { "device_slug": "studio", "path": "/Footage/take-3.mov" } } }
//! ```
//! answers the content uuid and one entry per artifact, each with the kind,
//! variant and format a client addresses it by, its size, who made it, and
//! the file on this device that holds it.

use std::{path::PathBuf, sync::Arc, time::UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::warn;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	domain::{sidecar::content_uuid_of_file, SdPath},
	infra::{
		api::SessionContext,
		query::{LibraryQuery, QueryError, QueryResult},
	},
	ops::indexing::metadata::refuse_dataless,
	service::{
		sidecars::SidecarRegistry,
		thumbs::{ThumbstripIdentity, TileIdentity},
	},
};

pub use crate::domain::sidecar::CORE_PRODUCER;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileSidecarsInput {
	pub path: SdPath,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileSidecarsOutput {
	/// The key of the file's content sidecars. `None` when the file could
	/// not be sampled, such as a cloud placeholder that reading would
	/// download.
	pub content_uuid: Option<Uuid>,
	pub sidecars: Vec<FileSidecar>,
}

/// One artifact derived from a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FileSidecar {
	/// A core kind (`thumb`, `thumbstrip`, `proxy`, `audio`, `transcript`,
	/// `shots`, `keyframe`) or the kind an extension declared.
	pub kind: String,
	pub variant: String,
	/// The artifact's file extension.
	pub format: String,
	/// Bytes stored. `None` for a tile only the raw hot cache holds.
	pub size: Option<u64>,
	/// [`CORE_PRODUCER`], or the id of the extension that wrote it.
	pub producer: String,
	pub address: SidecarAddress,
	/// The file on this device that holds the artifact. A thumbnail tile
	/// shares its file with the rest of its source's tiles.
	pub path: PathBuf,
}

/// How a client fetches an artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SidecarAddress {
	/// Served at `/sidecar/<library>/<content_uuid>/<kind>/<variant>.<format>`.
	/// The server marks sidecars immutable, so a URL carries `version`, the
	/// file's modification time in milliseconds, to see a regenerated one.
	Content { version: i64 },
	/// An extension's document, which only extensions read.
	Extension,
	/// Served at `/hot-thumb/<source_id>/<uuid>/<version>`.
	Thumbnail { tile: TileIdentity },
	/// Served at `/hot-thumbstrip/<source_id>/<uuid>/<version>`.
	Thumbstrip { thumbstrip: ThumbstripIdentity },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSidecarsQuery {
	input: FileSidecarsInput,
}

impl LibraryQuery for FileSidecarsQuery {
	type Input = FileSidecarsInput;
	type Output = Option<FileSidecarsOutput>;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
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
		let SdPath::Physical { device_slug, path } = &self.input.path else {
			return Ok(None);
		};
		if *device_slug != crate::device::get_current_device_slug() {
			return Ok(None);
		}
		let metadata = tokio::fs::metadata(path).await.map_err(|error| {
			QueryError::InvalidInput(format!("could not read the file: {error}"))
		})?;
		if !metadata.is_file() {
			return Ok(Some(FileSidecarsOutput {
				content_uuid: None,
				sidecars: Vec::new(),
			}));
		}

		let mut sidecars = Vec::new();
		let made = context.thumbs.made_for(path).await;
		if let Some(tile) = made.tile {
			sidecars.push(FileSidecar {
				kind: "thumb".into(),
				variant: "tile".into(),
				format: "webp".into(),
				size: tile.size,
				producer: CORE_PRODUCER.into(),
				address: SidecarAddress::Thumbnail {
					tile: tile.identity,
				},
				path: tile.file,
			});
		}
		if let Some(strip) = made.thumbstrip {
			sidecars.push(FileSidecar {
				kind: "thumbstrip".into(),
				variant: format!("{}x{}", strip.identity.columns, strip.identity.rows),
				format: "png".into(),
				size: strip.size,
				producer: CORE_PRODUCER.into(),
				address: SidecarAddress::Thumbstrip {
					thumbstrip: strip.identity,
				},
				path: strip.file,
			});
		}

		// Sampling a cloud placeholder would download it, so such a file
		// lists only what the thumbnail service holds.
		let content_uuid = match refuse_dataless(path).await {
			Ok(()) => content_uuid_of_file(path).await.ok(),
			Err(_) => None,
		};
		if let Some(content_uuid) = content_uuid {
			sidecars.extend(content_sidecars(&SidecarRegistry::of(&library), content_uuid).await);
		}

		Ok(Some(FileSidecarsOutput {
			content_uuid,
			sidecars,
		}))
	}
}

crate::register_library_query!(FileSidecarsQuery, "files.sidecars");

/// Every content sidecar of one content uuid across the library's sources,
/// core kinds first, each group sorted by kind, variant and format.
pub async fn content_sidecars(registry: &SidecarRegistry, content_uuid: Uuid) -> Vec<FileSidecar> {
	let mut sidecars: Vec<FileSidecar> = registry
		.list(content_uuid)
		.await
		.into_iter()
		.map(|listed| {
			let producer = listed.sidecar.producer().to_string();
			FileSidecar {
				kind: listed.sidecar.kind.as_str().to_string(),
				variant: listed.sidecar.variant.as_str().to_string(),
				format: listed.sidecar.format.extension().to_string(),
				size: Some(listed.size),
				address: if producer == CORE_PRODUCER {
					SidecarAddress::Content {
						version: listed.modified_ms,
					}
				} else {
					SidecarAddress::Extension
				},
				producer,
				path: listed.path,
			}
		})
		.collect();
	sidecars.sort_by(|a, b| {
		(
			a.producer != CORE_PRODUCER,
			&a.producer,
			&a.kind,
			&a.variant,
			&a.format,
		)
			.cmp(&(
				b.producer != CORE_PRODUCER,
				&b.producer,
				&b.kind,
				&b.variant,
				&b.format,
			))
	});
	sidecars
}
