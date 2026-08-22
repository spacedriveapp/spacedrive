//! Serialize a `u64` as a decimal string on the wire.
//!
//! JSON numbers are IEEE doubles in JavaScript, so anything above 2^53 comes
//! back rounded: a content version of 9128471848072465123 reaches a browser as
//! 9128471848072465000 and no longer matches the value the daemon stored.
//! Rust clients never see this because serde_json keeps `u64` exact, which is
//! what makes it a quiet bug rather than a loud one.
//!
//! Use this for any `u64` whose exact value a web client must round-trip.
//! Pair it with `#[specta(type = String)]` so the generated TypeScript says
//! what the wire actually carries.

use serde::{Deserialize, Deserializer, Serializer};

pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
	serializer.serialize_str(&value.to_string())
}

pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
	// Accept a bare number too, so a hand-written request or an older client
	// is not rejected over a formatting detail.
	#[derive(Deserialize)]
	#[serde(untagged)]
	enum Wire {
		Text(String),
		Number(u64),
	}

	match Wire::deserialize(deserializer)? {
		Wire::Text(text) => text.parse().map_err(serde::de::Error::custom),
		Wire::Number(value) => Ok(value),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde::{Deserialize, Serialize};

	#[derive(Debug, PartialEq, Serialize, Deserialize)]
	struct Wrapper {
		#[serde(with = "super")]
		version: u64,
	}

	#[test]
	fn a_version_past_the_double_range_survives_the_round_trip() {
		let wrapper = Wrapper {
			version: 9_128_471_848_072_465_123,
		};
		let json = serde_json::to_string(&wrapper).expect("serializes");
		assert_eq!(json, r#"{"version":"9128471848072465123"}"#);
		assert_eq!(
			serde_json::from_str::<Wrapper>(&json).expect("deserializes"),
			wrapper
		);
	}

	#[test]
	fn a_bare_number_is_still_accepted() {
		let wrapper: Wrapper = serde_json::from_str(r#"{"version":42}"#).expect("deserializes");
		assert_eq!(wrapper.version, 42);
	}
}
