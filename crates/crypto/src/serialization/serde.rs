// TODO(brxken128): test this, maybe move back to `serde_big_array` or similar

use crate::{primitives::ENCRYPTED_KEY_LEN, types::EncryptedKey};
use serde::ser::SerializeTuple;

impl serde::Serialize for EncryptedKey {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		let mut seq = serializer.serialize_tuple(ENCRYPTED_KEY_LEN)?;
		for b in self.inner() {
			seq.serialize_element(b)?;
		}

		seq.end()
	}
}

struct EncryptedKeyVisitor;

impl<'de> serde::de::Visitor<'de> for EncryptedKeyVisitor {
	type Value = [u8; ENCRYPTED_KEY_LEN];

	fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(formatter, "array of {ENCRYPTED_KEY_LEN} bytes in length")
	}

	fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
	where
		A: serde::de::SeqAccess<'de>,
	{
		let mut array = Vec::with_capacity(ENCRYPTED_KEY_LEN);

		while let Some(v) = seq.next_element()? {
			array.push(v);
		}

		array
			.try_into()
			.map_err(|e: Vec<u8>| serde::de::Error::invalid_length(e.len(), &self))
	}
}

impl<'de> serde::Deserialize<'de> for EncryptedKey {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		deserializer
			.deserialize_tuple(ENCRYPTED_KEY_LEN, EncryptedKeyVisitor)
			.map(Self::new)
	}
}
