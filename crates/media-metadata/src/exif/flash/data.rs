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
	fn from(value: FlashValue) -> Self {
		let numeric_value = value as u32;

		// Bit 0 indicates if flash fired
		let fired = (numeric_value & 0x01) != 0;

		// Bits 1-2 indicate return state
		let returned = match numeric_value & 0x06 {
			0x06 => Some(true),  // Returned
			0x02 => Some(false), // No return
			_ => None,           // No data
		};

		// Bit 6 indicates red-eye reduction
		let red_eye_reduction = if (numeric_value & 0x40) != 0 {
			Some(true)
		} else {
			None
		};

		let mode = FlashMode::from(numeric_value);

		let flash = Flash {
			mode,
			fired: Some(fired),
			returned,
			red_eye_reduction,
		};

		if flash == Flash::default() {
			None
		} else {
			Some(flash)
		}
	}
}
