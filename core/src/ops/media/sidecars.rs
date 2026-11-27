//! # Media sidecars of one file
//!
//! `media.sidecars` answers what a viewer needs to play and read one
//! recording: its transcript, its proxy and its shot list. Media sidecars are
//! keyed by content, and finding the content uuid reads samples of the file, so
//! the explorer listing never pays for it. A viewer asks for the one file it
//! shows, when it shows it.
//!
//! ## Example
//! ```json
//! { "path": { "Physical": { "device_slug": "studio", "path": "/Footage/take-3.mov" } } }
//! ```
//! answers `null` for a file on another device, and otherwise the content
//! uuid with whichever of the three sidecars exist.

use std::{path::Path, sync::Arc};

use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::warn;
use uuid::Uuid;

use super::{
	footage::{shot_descriptions, ShotDescription},
	shots::{content_uuid_of_video, read_shots, ShotList},
};
use crate::{
	context::CoreContext,
	domain::{ProxyResolution, SdPath},
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::{
		files::query::sidecars::{content_sidecars, SidecarAddress, CORE_PRODUCER},
		transcription::transcript::{read_transcript, Transcript},
	},
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MediaSidecarsInput {
	pub path: SdPath,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MediaSidecarsOutput {
	/// The key the sidecar server addresses this file's sidecars by.
	pub content_uuid: Uuid,
	/// `None` until the recording has been transcribed.
	pub transcript: Option<Transcript>,
	/// The largest proxy made for this content.
	pub proxy: Option<ProxySidecar>,
	/// `None` until shot detection has run.
	pub shots: Option<ShotList>,
	/// The Videos extension's description of each shot it has analyzed.
	pub descriptions: Vec<ShotDescription>,
}

/// A proxy file, named the way the sidecar server addresses it.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProxySidecar {
	pub variant: String,
	pub format: String,
	/// Modification time in milliseconds. The server marks sidecars
	/// immutable, so a URL carries this to see a regenerated proxy.
	pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaSidecarsQuery {
	input: MediaSidecarsInput,
}

impl LibraryQuery for MediaSidecarsQuery {
	type Input = MediaSidecarsInput;
	type Output = Option<MediaSidecarsOutput>;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
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
		// Sidecars are made on the device that holds the bytes, so another
		// device's file has none here.
		let SdPath::Physical { device_slug, path } = &self.input.path else {
			return Ok(None);
		};
		if *device_slug != crate::device::get_current_device_slug() {
			return Ok(None);
		}
		let content_uuid = content_uuid_of_video(path).await.map_err(|error| {
			QueryError::InvalidInput(format!("could not read the file: {error}"))
		})?;

		let library_path = library.path();
		// A sidecar that cannot be read leaves the others usable, and the
		// viewer plays the original without it.
		let transcript = read_transcript(library_path, content_uuid)
			.await
			.unwrap_or_else(|error| {
				warn!(%content_uuid, %error, "Could not read the transcript sidecar");
				None
			});
		let shots = read_shots(library_path, content_uuid)
			.await
			.unwrap_or_else(|error| {
				warn!(%content_uuid, %error, "Could not read the shots sidecar");
				None
			});
		let proxy = largest_proxy(library_path, content_uuid).await;
		let descriptions = shot_descriptions(library_path, content_uuid)
			.await
			.unwrap_or_else(|error| {
				warn!(%content_uuid, %error, "Could not read the scene descriptions");
				Vec::new()
			});

		Ok(Some(MediaSidecarsOutput {
			content_uuid,
			transcript,
			proxy,
			shots,
			descriptions,
		}))
	}
}

async fn largest_proxy(library_path: &Path, content_uuid: Uuid) -> Option<ProxySidecar> {
	let sidecars = content_sidecars(library_path, content_uuid)
		.await
		.unwrap_or_else(|error| {
			warn!(%content_uuid, %error, "Could not list the content sidecars");
			Vec::new()
		});
	[ProxyResolution::P1080, ProxyResolution::P720]
		.into_iter()
		.find_map(|resolution| {
			sidecars.iter().find(|sidecar| {
				sidecar.kind == "proxy"
					&& sidecar.producer == CORE_PRODUCER
					&& sidecar.variant == resolution.variant()
			})
		})
		.and_then(|sidecar| match sidecar.address {
			SidecarAddress::Content { version } => Some(ProxySidecar {
				variant: sidecar.variant.clone(),
				format: sidecar.format.clone(),
				version,
			}),
			_ => None,
		})
}

crate::register_library_query!(MediaSidecarsQuery, "media.sidecars");
