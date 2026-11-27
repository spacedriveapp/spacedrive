//! # Sidecars of one file
//!
//! `files.sidecars` lists everything this device has derived from one file,
//! whichever store holds it: the media sidecars in the library's
//! content-addressed tree (proxies, speech audio, transcripts, shot lists,
//! keyframes and the documents extensions write), and the thumbnail tile and
//! video scrub sheet the thumbnail service keeps by record. Nothing is
//! generated to answer it.
//!
//! The content tree is keyed by the candidate content uuid, which comes from
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

use std::{
	io,
	path::{Path, PathBuf},
	sync::Arc,
	time::UNIX_EPOCH,
};

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
	service::thumbs::{ThumbstripIdentity, TileIdentity},
};

/// The producer named for artifacts the core makes.
pub const CORE_PRODUCER: &str = "core";

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
			match content_sidecars(library.path(), content_uuid).await {
				Ok(found) => sidecars.extend(found),
				Err(error) => {
					warn!(%content_uuid, %error, "Could not list the content sidecars")
				}
			}
		}

		Ok(Some(FileSidecarsOutput {
			content_uuid,
			sidecars,
		}))
	}
}

crate::register_library_query!(FileSidecarsQuery, "files.sidecars");

/// Every sidecar in a library's content tree for one content uuid, core
/// kinds first, each group sorted by kind, variant and format.
///
/// Files starting with a dot are a writer's partial output and `.tmp` files
/// an extension document mid-rename; neither is a sidecar yet.
pub async fn content_sidecars(
	library_path: &Path,
	content_uuid: Uuid,
) -> io::Result<Vec<FileSidecar>> {
	let root = library_path
		.join("sidecars")
		.join(sd_sidecar_path::content_directory(&content_uuid));
	let mut sidecars = Vec::new();
	for (directory, name) in subdirectories(&root).await? {
		if name == "extensions" {
			for (extension, extension_id) in subdirectories(&directory).await? {
				for (kind_directory, kind) in subdirectories(&extension).await? {
					read_kind(
						&kind_directory,
						&kind,
						&extension_id,
						SidecarAddress::Extension,
						&mut sidecars,
					)
					.await?;
				}
			}
			continue;
		}
		let Some(kind) = sd_sidecar_path::directory_kind(&name) else {
			continue;
		};
		read_kind(
			&directory,
			kind,
			CORE_PRODUCER,
			SidecarAddress::Content { version: 0 },
			&mut sidecars,
		)
		.await?;
	}
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
	Ok(sidecars)
}

/// The directories directly under `path` with their names. A missing
/// directory has none.
async fn subdirectories(path: &Path) -> io::Result<Vec<(PathBuf, String)>> {
	let mut entries = match tokio::fs::read_dir(path).await {
		Ok(entries) => entries,
		Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
		Err(error) => return Err(error),
	};
	let mut found = Vec::new();
	while let Some(entry) = entries.next_entry().await? {
		let Ok(name) = entry.file_name().into_string() else {
			continue;
		};
		if entry.file_type().await?.is_dir() {
			found.push((entry.path(), name));
		}
	}
	Ok(found)
}

/// The sidecars in one kind's directory. A content sidecar's address takes
/// the file's modification time as its version.
async fn read_kind(
	directory: &Path,
	kind: &str,
	producer: &str,
	address: SidecarAddress,
	sidecars: &mut Vec<FileSidecar>,
) -> io::Result<()> {
	let mut entries = tokio::fs::read_dir(directory).await?;
	while let Some(entry) = entries.next_entry().await? {
		let Ok(name) = entry.file_name().into_string() else {
			continue;
		};
		if name.starts_with('.') || name.ends_with(".tmp") {
			continue;
		}
		let Some((variant, format)) = name.rsplit_once('.') else {
			continue;
		};
		let metadata = entry.metadata().await?;
		if !metadata.is_file() {
			continue;
		}
		let address = match address {
			SidecarAddress::Content { .. } => SidecarAddress::Content {
				version: metadata
					.modified()
					.ok()
					.and_then(|time| time.duration_since(UNIX_EPOCH).ok())
					.map_or(0, |since| since.as_millis() as i64),
			},
			ref other => other.clone(),
		};
		sidecars.push(FileSidecar {
			kind: kind.to_string(),
			variant: variant.to_string(),
			format: format.to_string(),
			size: Some(metadata.len()),
			producer: producer.to_string(),
			address,
			path: entry.path(),
		});
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::domain::{ContentSidecar, ProxyResolution, SidecarFormat};

	async fn write(path: PathBuf, bytes: &[u8]) {
		tokio::fs::create_dir_all(path.parent().unwrap())
			.await
			.unwrap();
		tokio::fs::write(path, bytes).await.unwrap();
	}

	#[tokio::test]
	async fn lists_core_and_extension_sidecars_and_skips_partials() {
		let library = tempfile::tempdir().unwrap();
		let uuid = Uuid::parse_str("abcd1234-5678-90ab-cdef-123456789012").unwrap();
		let proxy = ContentSidecar::proxy(uuid, ProxyResolution::P720).path_in(library.path());
		write(proxy.clone(), b"mp4").await;
		write(proxy.with_file_name(".720p.partial-1.mp4"), b"half").await;
		write(
			ContentSidecar::transcript(uuid, SidecarFormat::Json).path_in(library.path()),
			b"{}",
		)
		.await;
		let faces = library
			.path()
			.join("sidecars")
			.join(sd_sidecar_path::relative_path(
				&uuid,
				&sd_sidecar_path::extension_kind_directory("com.spacedrive.photos", "faces"),
				"default",
				"json",
			));
		write(faces.clone(), b"[]").await;
		write(faces.with_extension("json.tmp"), b"[").await;

		let found = content_sidecars(library.path(), uuid).await.unwrap();
		let names: Vec<_> = found
			.iter()
			.map(|s| {
				(
					s.producer.as_str(),
					s.kind.as_str(),
					s.variant.as_str(),
					s.format.as_str(),
				)
			})
			.collect();
		assert_eq!(
			names,
			[
				("core", "proxy", "720p", "mp4"),
				("core", "transcript", "default", "json"),
				("com.spacedrive.photos", "faces", "default", "json"),
			]
		);
		assert_eq!(found[0].size, Some(3));
		assert_eq!(found[0].path, proxy);
		assert!(matches!(found[0].address, SidecarAddress::Content { version } if version > 0));
		assert_eq!(found[2].address, SidecarAddress::Extension);
	}

	#[tokio::test]
	async fn content_without_sidecars_lists_nothing() {
		let library = tempfile::tempdir().unwrap();
		assert!(content_sidecars(library.path(), Uuid::new_v4())
			.await
			.unwrap()
			.is_empty());
	}
}
