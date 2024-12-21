use std::path::{Path, PathBuf};

use sd_core_prisma_helpers::CasId;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

pub const EPHEMERAL_DIR: &str = "ephemeral";
pub const THUMBNAIL_CACHE_DIR_NAME: &str = "thumbnails";
pub const WEBP_EXTENSION: &str = "webp";

#[derive(Debug, Serialize, Deserialize, Type, Clone, Copy)]
pub enum ThumbnailKind {
	Ephemeral,
	Indexed(Uuid),
}

impl ThumbnailKind {
	pub fn compute_path(&self, data_directory: impl AsRef<Path>, cas_id: &CasId<'_>) -> PathBuf {
		let mut thumb_path = get_thumbnails_directory(data_directory);
		match self {
			Self::Ephemeral => thumb_path.push(EPHEMERAL_DIR),
			Self::Indexed(library_id) => {
				thumb_path.push(library_id.to_string());
			}
		}
		thumb_path.push(get_shard_hex(cas_id));
		thumb_path.push(cas_id.as_str());
		thumb_path.set_extension(WEBP_EXTENSION);

		thumb_path
	}
}

pub fn get_thumbnails_directory(data_directory: impl AsRef<Path>) -> PathBuf {
	data_directory.as_ref().join(THUMBNAIL_CACHE_DIR_NAME)
}

/// This type is used to pass the relevant data to the frontend so it can request the thumbnail.
/// Tt supports extending the shard hex to support deeper directory structures in the future
#[derive(Debug, Serialize, Deserialize, Type, Clone)]
pub struct ThumbKey {
	pub shard_hex: String,
	pub cas_id: CasId<'static>,
	pub base_directory_str: String,
}

impl ThumbKey {
	#[must_use]
	pub fn new(cas_id: CasId<'static>, kind: &ThumbnailKind) -> Self {
		Self {
			shard_hex: get_shard_hex(&cas_id).to_string(),
			cas_id,
			base_directory_str: match kind {
				ThumbnailKind::Ephemeral => String::from(EPHEMERAL_DIR),
				ThumbnailKind::Indexed(library_id) => library_id.to_string(),
			},
		}
	}

	#[must_use]
	pub fn new_indexed(cas_id: CasId<'static>, library_id: Uuid) -> Self {
		Self {
			shard_hex: get_shard_hex(&cas_id).to_string(),
			cas_id,
			base_directory_str: library_id.to_string(),
		}
	}

	#[must_use]
	pub fn new_ephemeral(cas_id: CasId<'static>) -> Self {
		Self {
			shard_hex: get_shard_hex(&cas_id).to_string(),
			cas_id,
			base_directory_str: String::from(EPHEMERAL_DIR),
		}
	}
}

/// The practice of dividing files into hex coded folders, often called "sharding,"
/// is mainly used to optimize file system performance. File systems can start to slow down
/// as the number of files in a directory increases. Thus, it's often beneficial to split
/// files into multiple directories to avoid this performance degradation.
///
/// `get_shard_hex` takes a `cas_id` (a hexadecimal hash) as input and returns the first
/// three characters of the hash as the directory name. Because we're using these first
/// three characters of a the hash, this will give us 4096 (16^3) possible directories,
/// named 000 to fff.
#[inline]
#[must_use]
pub fn get_shard_hex<'cas_id>(cas_id: &'cas_id CasId<'cas_id>) -> &'cas_id str {
	// Use the first three characters of the hash as the directory name
	&cas_id.as_str()[0..3]
}
