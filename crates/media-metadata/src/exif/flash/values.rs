use std::fmt::Display;

// https://exiftool.org/TagNames/EXIF.html scroll to bottom to get codds
#[derive(
	Clone, Copy, Default, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub enum FlashValue {
	#[default]
	Unknown,
	NoFire,
	Fired,
	FiredNoReturn,
	FiredReturn,
	OnNoFire,
	OnFired,
	OnNoReturn,
	OnReturn,
	OffNoFire,
	OffNoFireNoReturn,
	AutoNoFire,
	AutoFired,
	AutoFiredNoReturn,
	AutoFiredReturn,
	NoFlashFunction,
	OffNoFlashFunction,
	FiredRedEyeReduction,
	FiredRedEyeReductionNoReturn,
	FiredRedEyeReductionReturn,
	OnRedEyeReduction,
	OnRedEyeReductionNoReturn,
	OnRedEyeReductionReturn,
	OffRedEyeReduction,
	AutoNoFireRedEyeReduction,
	AutoFiredRedEyeReduction,
	AutoFiredRedEyeReductionNoReturn,
	AutoFiredRedEyeReductionReturn,
}

impl FlashValue {
	#[must_use]
	pub fn new(value: u32) -> Self {
		value.into()
	}
}

impl From<u32> for FlashValue {
	fn from(value: u32) -> Self {
		match value {
			0x00 => Self::NoFire,
			0x01 => Self::Fired,
			0x05 => Self::FiredNoReturn,
			0x07 => Self::FiredReturn,
			0x08 => Self::OnNoFire,
			0x09 => Self::OnFired,
			0x0d => Self::OnNoReturn,
			0x0f => Self::OnReturn,
			0x10 => Self::OffNoFire,
			0x14 => Self::OffNoFireNoReturn,
			0x18 => Self::AutoNoFire,
			0x19 => Self::AutoFired,
			0x1d => Self::AutoFiredNoReturn,
			0x1f => Self::AutoFiredReturn,
			0x20 => Self::NoFlashFunction,
			0x30 => Self::OffNoFlashFunction,
			0x41 => Self::FiredRedEyeReduction,
			0x45 => Self::FiredRedEyeReductionNoReturn,
			0x47 => Self::FiredRedEyeReductionReturn,
			0x49 => Self::OnRedEyeReduction,
			0x4d => Self::OnRedEyeReductionNoReturn,
			0x4f => Self::OnRedEyeReductionReturn,
			0x50 => Self::OffRedEyeReduction,
			0x58 => Self::AutoNoFireRedEyeReduction,
			0x59 => Self::AutoFiredRedEyeReduction,
			0x5d => Self::AutoFiredRedEyeReductionNoReturn,
			0x5f => Self::AutoFiredRedEyeReductionReturn,
			_ => Self::default(),
		}
	}
}

impl Display for FlashValue {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Unknown => f.write_str("Flash data was present but we were unable to parse it"),
			Self::NoFire => f.write_str("Flash didn't fire"),
			Self::Fired => f.write_str("Flash fired"),
			Self::FiredNoReturn => f.write_str("Flash fired but no return detected"),
			Self::FiredReturn => f.write_str("Flash fired and return was detected"),
			Self::OnNoFire => f.write_str("Flash was enabled but not fired"),
			Self::OnFired => f.write_str("Flash was enabled and fired"),
			Self::OnNoReturn => f.write_str("Flash was enabled but no return detected"),
			Self::OnReturn => f.write_str("Flash was enabled and return was detected"),
			Self::OffNoFire => f.write_str("Flash was disabled"),
			Self::OffNoFireNoReturn => {
				f.write_str("FLash was disabled, did not fire and no return was detected")
			}
			Self::AutoNoFire => f.write_str("Auto was enabled but flash did not fire"),
			Self::AutoFired => f.write_str("Auto was enabled and fired"),
			Self::AutoFiredNoReturn => {
				f.write_str("Auto was enabled and fired, no return was detected")
			}
			Self::AutoFiredReturn => f.write_str("Auto was enabled and fired, return was detected"),
			Self::NoFlashFunction => f.write_str("Device has no flash function"),
			Self::OffNoFlashFunction => f.write_str("Off as device has no flash function"),
			Self::FiredRedEyeReduction => f.write_str("Flash fired with red eye reduction"),
			Self::FiredRedEyeReductionNoReturn => {
				f.write_str("Flash fired with red eye reduction, no return was detected")
			}
			Self::FiredRedEyeReductionReturn => {
				f.write_str("Flash fired with red eye reduction, return was detecteed")
			}
			Self::OnRedEyeReduction => f.write_str("Flash was enabled with red eye reduction"),
			Self::OnRedEyeReductionNoReturn => {
				f.write_str("Flash was enabled with red eye reduction, no return was detected")
			}
			Self::OnRedEyeReductionReturn => {
				f.write_str("Flash was enabled with red eye reduction, return was detected")
			}
			Self::OffRedEyeReduction => {
				f.write_str("Flash was disabled, but red eye reduction was enabled")
			}
			Self::AutoNoFireRedEyeReduction => {
				f.write_str("Auto was enabled but didn't fire, and red eye reduction was used")
			}
			Self::AutoFiredRedEyeReduction => {
				f.write_str("Auto was enabled and fired, and red eye reduction was used")
			}
			Self::AutoFiredRedEyeReductionNoReturn => f.write_str(
				"Auto was enabled and fired, and red eye reduction was enabled but did not return",
			),
			Self::AutoFiredRedEyeReductionReturn => f.write_str(
				"Auto was enabled and fired, and red eye reduction was enabled and returned",
			),
		}
	}
}
