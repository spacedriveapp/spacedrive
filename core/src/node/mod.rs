use chrono::{DateTime, Utc};
use int_enum::IntEnum;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

mod config;

pub use config::*;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LibraryNode {
	pub uuid: String,
	pub name: String,
	pub platform: Platform,
	pub tcp_address: String,
	#[ts(type = "string")]
	pub last_seen: DateTime<Utc>,
	#[ts(type = "string")]
	pub last_synchronized: DateTime<Utc>,
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
