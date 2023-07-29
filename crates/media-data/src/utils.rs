use crate::{Error, Result};

pub fn to_slice_option<T: serde::Serialize + serde::de::DeserializeOwned>(
	value: &T,
) -> Option<Vec<u8>> {
	serde_json::to_vec(value).ok()
}

pub fn from_slice_option_to_res<T: serde::Serialize + serde::de::DeserializeOwned>(
	value: Option<Vec<u8>>,
) -> Result<T> {
	value.map_or(Err(Error::Conversion), |x| {
		serde_json::from_slice(&x).map_err(|_| Error::Conversion)
	})
}

pub fn from_slice_option_to_option<T: serde::Serialize + serde::de::DeserializeOwned>(
	value: Option<Vec<u8>>,
) -> Option<T> {
	value
		.map(|x| serde_json::from_slice(&x).ok())
		.unwrap_or_default()
}
