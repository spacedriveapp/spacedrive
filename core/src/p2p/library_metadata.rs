use std::collections::HashMap;

use sd_p2p::Metadata;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
pub struct LibraryMetadata {}

impl Metadata for LibraryMetadata {
	fn to_hashmap(self) -> HashMap<String, String> {
		HashMap::with_capacity(0)
	}

	fn from_hashmap(_: &HashMap<String, String>) -> Result<Self, String>
	where
		Self: Sized,
	{
		Ok(Self {})
	}
}
