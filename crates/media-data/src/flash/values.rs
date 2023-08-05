use std::fmt::Display;

use crate::{Error, Result};

// https://exiftool.org/TagNames/EXIF.html scroll to bottom to get codds
#[derive(
	Default, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type,
)]
#[repr(u32)]
pub enum FlashValue {
	#[default]
	NoFire = 0x00,
	Fired = 0x01,
	FiredNoReturn = 0x05,
	FiredReturn = 0x07,
	OnNoFire = 0x08,
	OnFired = 0x09,
	OnNoReturn = 0x0d,
	OnReturn = 0x0f,
	OffNoFire = 0x10,
	OffNoFireNoReturn = 0x14,
	AutoNoFire = 0x18,
	AutoFired = 0x19,
	AutoFiredNoReturn = 0x1d,
	AutoFiredReturn = 0x1f,
	NoFlashFunction = 0x20,
	OffNoFlashFunction = 0x30,
	FiredRedEyeReduction = 0x41,
	FiredRedEyeReductionNoReturn = 0x45,
	FiredRedEyeReductionReturn = 0x47,
	OnRedEyeReduction = 0x49,
	OnRedEyeReductionNoReturn = 0x4d,
	OnRedEyeReductionReturn = 0x4f,
	OffRedEyeReduction = 0x50,
	AutoNoFireRedEyeReduction = 0x58,
	AutoFiredRedEyeReduction = 0x59,
	AutoFiredRedEyeReductionNoReturn = 0x5d,
	AutoFiredRedEyeReductionReturn = 0x5f,
}

impl FlashValue {
	#[must_use]
	pub fn new(value: u32) -> Option<Self> {
		let x: Result<Self> = value.try_into();
		x.ok()
	}
}

impl TryFrom<u32> for FlashValue {
	type Error = Error;

	fn try_from(value: u32) -> std::result::Result<Self, Self::Error> {
		let res = match value {
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
			_ => return Err(Error::Conversion),
		};

		Ok(res)
	}
}

impl Display for FlashValue {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
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
