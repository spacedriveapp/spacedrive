#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Flash {
	pub enabled: Option<bool>,
	pub auto: Option<bool>,
	pub red_eye: Option<bool>,
}

// https://exiftool.org/TagNames/EXIF.html scroll to bottom to get opcodes
