use exif::Tag;

use super::FlashValue;
use crate::exif::{flash::consts::FLASH_MODES, ExifReader};

#[derive(
	Default, Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub struct Flash {
	/// Specifies how flash was used (on, auto, off, forced, onvalid)
	///
	/// [`FlashMode::Unknown`] isn't a valid EXIF state, but it's included as the default,
	///  just in case we're unable to correctly match it to a known (valid) state.
	///
	/// This type should only ever be evaluated if flash EXIF data is present, so having this as a non-option shouldn't be an issue.
	pub mode: FlashMode,
	/// Did the flash actually fire?
	pub fired: Option<bool>,
	/// Did flash return to the camera? (Unsure of the meaning)
	pub returned: Option<bool>,
	/// Was red eye reduction used?
	pub red_eye_reduction: Option<bool>,
}

impl Flash {
	#[must_use]
	pub fn from_reader(reader: &ExifReader) -> Option<Self> {
		let value = reader.get_tag_int(Tag::Flash)?;
		FlashValue::from(value).into()
	}
}

#[derive(
	Default, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub enum FlashMode {
	/// The data is present, but we're unable to determine what they mean
	#[default]
	Unknown,
	/// `FLash` was on
	On,
	/// Flash was off
	Off,
	/// Flash was set to automatically fire in certain conditions
	Auto,
	/// Flash was forcefully fired
	Forced,
}

impl From<u32> for FlashMode {
	fn from(value: u32) -> Self {
		FLASH_MODES
			.into_iter()
			.find_map(|(mode, slice)| slice.contains(&value).then_some(mode))
			.unwrap_or_default()
	}
}

impl From<FlashValue> for Option<Flash> {
	// TODO(brxken128): This can be heavily optimized with bitwise AND
	// e.g. to see if flash was fired, `(value & 1) != 0`
	// or to see if red eye reduction was enabled, `(value & 64) != 0`
	// May not be worth it as some states may be invalid according to `https://www.awaresystems.be/imaging/tiff/tifftags/privateifd/exif/flash.html`
	fn from(value: FlashValue) -> Self {
		#[allow(clippy::as_conversions)]
		let mut data = Flash {
			mode: FlashMode::from(value as u32),
			..Default::default()
		};

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
			FlashValue::NoFlashFunction | FlashValue::OffNoFlashFunction | FlashValue::Unknown => {
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

		// this means it had a value of Flash::NoFlashFunctionality
		if data == Flash::default() {
			None
		} else {
			Some(data)
		}
	}
}
