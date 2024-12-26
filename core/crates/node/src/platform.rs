use sd_core_shared_types::Platform;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlatformDetectionError {
	#[error("Invalid platform int: {0}")]
	InvalidPlatformInt(u8),
}

impl TryFrom<u8> for Platform {
	type Error = PlatformDetectionError;

	fn try_from(value: u8) -> Result<Self, Self::Error> {
		let s = match value {
			0 => Self::Unknown,
			1 => Self::Windows,
			2 => Self::MacOS,
			3 => Self::Linux,
			4 => Self::IOS,
			5 => Self::Android,
			_ => return Err(PlatformDetectionError::InvalidPlatformInt(value)),
		};

		Ok(s)
	}
}

impl From<Platform> for u8 {
	fn from(platform: Platform) -> Self {
		platform as u8
	}
}
