use sd_core_prisma_helpers::CasId;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

pub const EPHEMERAL_DIR: &str = "ephemeral";

/// This type is used to pass the relevant data to the frontend so it can request the thumbnail.
/// It supports extending the shard hex to support deeper directory structures in the future
#[derive(Debug, Serialize, Deserialize, Type, Clone)]
pub struct ThumbKey {
	pub shard_hex: String,
	pub cas_id: CasId<'static>,
	pub base_directory_str: String,
}

impl ThumbKey {
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

/// Get the shard hex for a given CasId
/// The practice of dividing files into hex coded folders, often called "sharding,"
/// is mainly used to optimize file system performance. File systems can start to slow down
/// as the number of files in a directory increases. Thus, it's often beneficial to split
/// files into multiple directories to avoid this performance degradation.
fn get_shard_hex<'a>(cas_id: &'a CasId<'a>) -> &'a str {
	&cas_id.as_str()[..3]
}
