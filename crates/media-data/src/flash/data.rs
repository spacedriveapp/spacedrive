use super::FlashValue;
use crate::{
	flash::consts::{
		DIDNT_FIRE, DIDNT_RETURN, FIRED, FLASH_MODES, NO_FLASH_FUNCTIONALITY, RER_ENABLED, RETURNED,
	},
	ExifReader,
};

#[derive(
	Default, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub struct Flash {
	/// Specifies how flash was used
	pub mode: FireMode,
	/// Did the flash actually fire?
	pub fired: Option<bool>,
	/// Did flash return to the camera? (Unsure of the meaning)
	pub returned: Option<bool>,
	// /// Was flash set to auto?
	// pub auto: Option<bool>,
	/// Was red eye reduction used?
	pub red_eye_reduction: Option<bool>,
}

impl Flash {
	#[must_use]
	pub fn from_reader(reader: &ExifReader) -> Option<Self> {
		let value = reader.get_flash_int()?;
		FlashValue::try_from(value).ok()?.into()
	}
}

// I'm unsure whether we should have these^^^ (and some others) as non-options,
// but not all features are available on all devices and we can't know which have them or not.

#[derive(
	Default, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub enum FireMode {
	On,
	Off,
	Auto,
	Forced,
	#[default]
	Unknown,
}

impl From<u32> for FireMode {
	fn from(value: u32) -> Self {
		FLASH_MODES
			.into_iter()
			.find_map(|(mode, slice)| slice.contains(&value).then_some(mode))
			.unwrap_or_default()
	}
}

impl From<FlashValue> for Option<Flash> {
	// TODO(brxken128): This can be heavily optimised with bitwise AND
	// e.g. to see if flash was fired, `(value & 1) != 0`
	// or to see if red eye reduction was enabled, `(value & 64) != 0`
	// May not be worth it as some states may be invalid according to `https://www.awaresystems.be/imaging/tiff/tifftags/privateifd/exif/flash.html`
	fn from(value: FlashValue) -> Self {
		let mut data = Flash::default();

		#[allow(clippy::match_same_arms)]
		match value {
			FlashValue::Fired => {
				data.fired = Some(true);
			}
			FlashValue::NoFire => {
				data.fired = Some(false);
			}
			FlashValue::FiredReturn => {
				data.fired = Some(true);
				data.returned = Some(true);
			}
			FlashValue::FiredNoReturn => {
				data.fired = Some(true);
				data.returned = Some(false);
			}
			FlashValue::AutoFired => {
				data.fired = Some(true);
			}
			FlashValue::AutoFiredNoReturn => {
				data.fired = Some(true);
				data.returned = Some(false);
			}
			FlashValue::OffNoFire => data.fired = Some(false),
			FlashValue::AutoNoFire => data.fired = Some(false),
			FlashValue::NoFlashFunction | FlashValue::OffNoFlashFunction => {
				data = Flash::default();
			}
			FlashValue::AutoFiredRedEyeReduction => {
				data.fired = Some(true);
				data.red_eye_reduction = Some(true);
			}
			FlashValue::AutoFiredRedEyeReductionNoReturn => {
				data.fired = Some(true);
				data.red_eye_reduction = Some(true);
				data.returned = Some(false);
			}
			FlashValue::AutoFiredRedEyeReductionReturn => {
				data.fired = Some(true);
				data.red_eye_reduction = Some(true);
				data.returned = Some(true);
			}
			FlashValue::OnFired => {
				data.fired = Some(true);
			}
			FlashValue::OnNoFire => {
				data.fired = Some(false);
			}
			FlashValue::AutoFiredReturn => {
				data.fired = Some(true);
				data.returned = Some(true);
			}
			FlashValue::OnReturn => {
				data.returned = Some(true);
			}
			FlashValue::OnNoReturn => data.returned = Some(false),
			FlashValue::AutoNoFireRedEyeReduction => {
				data.fired = Some(false);
				data.red_eye_reduction = Some(true);
			}
			FlashValue::OffNoFireNoReturn => {
				data.fired = Some(false);
				data.returned = Some(false);
			}
			FlashValue::OffRedEyeReduction => data.red_eye_reduction = Some(true),
			FlashValue::OnRedEyeReduction => data.red_eye_reduction = Some(true),
			FlashValue::FiredRedEyeReductionNoReturn => {
				data.fired = Some(true);
				data.red_eye_reduction = Some(true);
				data.returned = Some(false);
			}
			FlashValue::FiredRedEyeReduction => {
				data.fired = Some(true);
				data.red_eye_reduction = Some(true);
			}
			FlashValue::FiredRedEyeReductionReturn => {
				data.fired = Some(true);
				data.red_eye_reduction = Some(true);
				data.returned = Some(false);
			}
			FlashValue::OnRedEyeReductionReturn => {
				data.red_eye_reduction = Some(true);
				data.returned = Some(true);
			}
			FlashValue::OnRedEyeReductionNoReturn => {
				data.red_eye_reduction = Some(true);
				data.returned = Some(false);
			}
		}

		#[allow(clippy::as_conversions)]
		{
			data.mode = FireMode::from(value as u32);
		}

		// this means it had a value of Flash::NoFlashFunctionality
		if data != Flash::default() {
			Some(data)
		} else {
			None
		}
	}
}
