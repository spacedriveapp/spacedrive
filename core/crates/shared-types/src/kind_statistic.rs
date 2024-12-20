use serde::{Deserialize, Serialize};
use specta::Type;

// Used to total number of files of a kind
#[derive(Debug, Serialize, Deserialize, Type, Default, Clone)]
pub struct KindStatistic {
	kind: i32,
	name: String,
	count: (u32, u32),
	total_bytes: (u32, u32),
}
