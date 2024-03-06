use uuid::Uuid;

pub mod db;
pub mod error;

/// Combines an iterator of `T` and an iterator of `Option<T>`,
/// removing any `None` values in the process
pub fn chain_optional_iter<T>(
	required: impl IntoIterator<Item = T>,
	optional: impl IntoIterator<Item = Option<T>>,
) -> Vec<T> {
	required
		.into_iter()
		.map(Some)
		.chain(optional)
		.flatten()
		.collect()
}

#[must_use]
pub fn uuid_to_bytes(uuid: Uuid) -> Vec<u8> {
	uuid.as_bytes().to_vec()
}

#[must_use]
pub fn from_bytes_to_uuid(bytes: &[u8]) -> Uuid {
	Uuid::from_slice(bytes).expect("corrupted uuid in database")
}

#[macro_export]
macro_rules! msgpack {
	(null) => {
		::rmpv::Value::Nil
	};
	($e:expr) => {
		::rmpv::ext::to_value(&$e).expect("failed to serialize msgpack")
	}
}
