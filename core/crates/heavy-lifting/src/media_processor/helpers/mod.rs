pub mod exif_media_data;
pub mod ffmpeg_media_data;
pub mod thumbnailer;

#[must_use]
fn from_slice_option_to_option<T: serde::Serialize + serde::de::DeserializeOwned>(
	value: Option<Vec<u8>>,
) -> Option<T> {
	value
		.map(|x| serde_json::from_slice(&x).ok())
		.unwrap_or_default()
}
