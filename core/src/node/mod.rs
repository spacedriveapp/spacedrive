use chrono::{DateTime, Utc};
use int_enum::IntEnum;
use rspc::Type;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod config;
use crate::prisma::node;
pub use config::*;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryNode {
	pub uuid: Uuid,
	pub name: String,
	pub platform: Platform,
	pub last_seen: DateTime<Utc>,
}

impl From<node::Data> for LibraryNode {
	fn from(data: node::Data) -> Self {
		Self {
			uuid: Uuid::from_slice(&data.pub_id).unwrap(),
			name: data.name,
			platform: IntEnum::from_int(data.platform).unwrap(),
			last_seen: data.last_seen.into(),
		}
	}
}

impl From<Box<node::Data>> for LibraryNode {
	fn from(data: Box<node::Data>) -> Self {
		Self::from(*data)
	}
}

#[allow(clippy::upper_case_acronyms)]
#[repr(i32)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Eq, PartialEq, IntEnum)]
pub enum Platform {
	Unknown = 0,
	Windows = 1,
	MacOS = 2,
	Linux = 3,
	IOS = 4,
	Android = 5,
}
