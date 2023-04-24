use crate::{prisma::node, NodeError};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

mod config;
pub mod peer_request;

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
			platform: Platform::try_from(data.platform).unwrap(),
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Eq, PartialEq)]
pub enum Platform {
	Unknown = 0,
	Windows = 1,
	MacOS = 2,
	Linux = 3,
	IOS = 4,
	Android = 5,
}

impl TryFrom<i32> for Platform {
	type Error = NodeError;

	fn try_from(value: i32) -> Result<Self, Self::Error> {
		let s = match value {
			0 => Self::Unknown,
			1 => Self::Windows,
			2 => Self::MacOS,
			3 => Self::Linux,
			4 => Self::IOS,
			5 => Self::Android,
			_ => return Err(NodeError::InvalidPlatformInt(value)),
		};

		Ok(s)
	}
}
