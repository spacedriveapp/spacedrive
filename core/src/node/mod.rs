use chrono::{DateTime, Utc};
use int_enum::IntEnum;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
mod config;
use crate::prisma::node;
pub use config::*;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LibraryNode {
	pub uuid: String,
	pub name: String,
	pub platform: Platform,
	pub last_seen: DateTime<Utc>,
}

impl Into<LibraryNode> for node::Data {
	fn into(self) -> LibraryNode {
		LibraryNode {
			uuid: self.pub_id,
			name: self.name,
			platform: IntEnum::from_int(self.platform).unwrap(),
			last_seen: self.last_seen.into(),
		}
	}
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, Eq, PartialEq, IntEnum)]
#[ts(export)]
pub enum Platform {
	Unknown = 0,
	Windows = 1,
	MacOS = 2,
	Linux = 3,
	IOS = 4,
	Android = 5,
}
