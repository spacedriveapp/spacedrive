use serde::{Deserialize, Serialize};
use specta::Type;

use super::{metadata::Metadata, stream::Stream};

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct Program {
	pub id: i32,
	pub name: Option<String>,
	pub streams: Vec<Stream>,
	pub metadata: Metadata,
}
