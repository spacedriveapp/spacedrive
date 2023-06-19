use crate::NodeError;
use serde::{Deserialize, Serialize};
use specta::Type;

mod config;

pub use config::*;

#[allow(clippy::upper_case_acronyms)]
#[repr(u8)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Eq, PartialEq)]
pub enum Platform {
	Unknown = 0,
	Windows = 1,
	MacOS = 2,
	Linux = 3,
	IOS = 4,
	Android = 5,
}

impl Platform {
	#[allow(unreachable_code)]
	pub fn current() -> Self {
		#[cfg(target_os = "windows")]
		return Self::Windows;

		#[cfg(target_os = "macos")]
		return Self::MacOS;

		#[cfg(target_os = "linux")]
		return Self::Linux;

		#[cfg(target_os = "ios")]
		return Self::IOS;

		#[cfg(target_os = "android")]
		return Self::Android;

		Self::Unknown
	}
}

impl TryFrom<u8> for Platform {
	type Error = NodeError;

	fn try_from(value: u8) -> Result<Self, Self::Error> {
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
