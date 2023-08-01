use crate::{
	flash::consts::{DIDNT_FIRE, DIDNT_RETURN, FIRED, RER_ENABLED, RETURNED},
	Error, ExifReader,
};
use std::fmt::Display;

use super::consts::{FLASH_MODES, NO_FLASH_FUNCTIONALITY};

#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Flash {
	/// Specifies how flash was used
	pub mode: FireMode,
	/// Did the flash actually fire?
	pub fired: Option<bool>,
	/// Did flash return to the camera? (Unsure of the meaning)
	pub returned: Option<bool>,
	/// Was flash set to auto?
	pub auto: Option<bool>,
	/// Was red eye reduction used?
	pub red_eye_reduction: Option<bool>,
}

impl Flash {
	#[must_use]
	pub fn from_reader(reader: &ExifReader) -> Option<Self> {
		let value = reader.get_flash_int().unwrap_or_default();
		let flash_value = FlashValue::try_from(value).unwrap_or_default();

		#[allow(clippy::as_conversions)]
		if NO_FLASH_FUNCTIONALITY.contains(&(flash_value as u32)) {
			return None;
		}
		Some(Flash::from(flash_value))
	}
}

// I'm unsure whether we should have these^^^ (and some others) as non-options,
// but not all features are available on all devices and we can't know which have them or not.

#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum FireMode {
	On,
	Off,
	Auto,
	Forced,
	#[default]
	None,
}

impl From<&u32> for FireMode {
	#[allow(unused_assignments)]
	fn from(value: &u32) -> Self {
		FLASH_MODES
			.into_iter()
			.find_map(|(mode, slice)| slice.contains(value).then_some(mode))
			.unwrap_or_default()
	}
}

impl From<FlashValue> for Flash {
	// TODO(brxken128): This can be heavily optimised with bitwise AND
	// e.g. to see if flash was fired, `(value & 1) != 0`
	// or to see if red eye reduction was enabled, `(value & 64) != 0`
	// May not be worth it as some states may be invalid according to `https://www.awaresystems.be/imaging/tiff/tifftags/privateifd/exif/flash.html`
	fn from(value: FlashValue) -> Self {
		#[allow(clippy::as_conversions)]
		let value = &(value as u32);
		let mut data = Self::default();

		data.mode = FireMode::from(value);

		data.fired = FIRED
			.contains(value)
			.then_some(true)
			.map_or_else(|| DIDNT_FIRE.contains(value).then_some(false), Some);

		data.red_eye_reduction = RER_ENABLED.contains(value).then_some(true);

		data.returned = RETURNED.contains(value).then_some(true); // if this is `None` then flash can't have fired (except in 0x14?)
		data.returned = DIDNT_RETURN.contains(value).then_some(false); // if this is `None` then flash can't have fired (except in 0x14?)

		todo!()
	}
}

// https://exiftool.org/TagNames/EXIF.html scroll to bottom to get codds
#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
#[repr(u32)]
enum FlashValue {
	#[default]
	DidntFire = 0x00,
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

impl TryFrom<u32> for FlashValue {
	type Error = Error;

	fn try_from(value: u32) -> Result<Self, Self::Error> {
		let res = match value {
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
			_ => Self::DidntFire,
		};

		Ok(res)
	}
}

impl Display for FlashValue {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::DidntFire => f.write_str("Flash didn't fire"),
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
