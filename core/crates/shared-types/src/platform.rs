use serde::{Deserialize, Serialize};
use specta::Type;

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
