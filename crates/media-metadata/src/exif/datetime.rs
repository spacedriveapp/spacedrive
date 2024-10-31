use super::{
	consts::{OFFSET_TAGS, TIME_TAGS},
	ExifReader,
};
use chrono::{DateTime, FixedOffset, NaiveDateTime};
use serde::{
	de::{self, Visitor},
	Deserialize, Deserializer,
};

pub const UTC_FORMAT_STR: &str = "%F %T %z";
pub const NAIVE_FORMAT_STR: &str = "%F %T";

/// This can be either naive with no TZ (`YYYY-MM-DD HH-MM-SS`) or UTC (`YYYY-MM-DD HH-MM-SS ±HHMM`),
/// where `±HHMM` is the timezone data. It may be negative if West of the Prime Meridian, or positive if East.
#[derive(Clone, Debug, PartialEq, Eq, specta::Type)]
#[serde(untagged)]
pub enum MediaDate {
	Naive(NaiveDateTime),
	Utc(DateTime<FixedOffset>),
}

impl MediaDate {
	/// This iterates over all 3 pairs of time/offset tags in an attempt to create a UTC time.
	///
	/// If the above fails, we fall back to Naive time - if that's not present this is `Undefined`.
	#[must_use]
	pub fn from_reader(reader: &ExifReader) -> Option<Self> {
		let z = TIME_TAGS
			.into_iter()
			.zip(OFFSET_TAGS)
			.filter_map(|(time_tag, offset_tag)| {
				let time = reader.get_tag::<String>(time_tag);
				let offset = reader.get_tag::<String>(offset_tag);

				if let (Some(t), Some(o)) = (time.clone(), offset) {
					DateTime::parse_from_str(&(format!("{t} {o}")), UTC_FORMAT_STR)
						.ok()
						.map(Self::Utc)
				} else if let Some(t) = time {
					NaiveDateTime::parse_from_str(&t, NAIVE_FORMAT_STR)
						.map_or(None, |x| Some(Self::Naive(x)))
				} else {
					None
				}
			})
			.collect::<Vec<_>>();

		z.iter()
			.find(|x| matches!(x, Self::Utc(_) | Self::Naive(_)))
			.cloned()
	}

	/// Returns the amount of non-leap seconds since the Unix Epoch (1970-01-01T00:00:00+00:00)
	///
	/// This is for search ordering/sorting
	#[must_use]
	pub const fn unix_timestamp(&self) -> i64 {
		match self {
			Self::Utc(t) => t.timestamp(),
			Self::Naive(t) => t.and_utc().timestamp(),
		}
	}
}

impl serde::Serialize for MediaDate {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		match self {
			Self::Utc(t) => serializer.serialize_str(&t.format(UTC_FORMAT_STR).to_string()),
			Self::Naive(t) => serializer.serialize_str(&t.format(NAIVE_FORMAT_STR).to_string()),
		}
	}
}

struct MediaDateVisitor;

impl Visitor<'_> for MediaDateVisitor {
	type Value = MediaDate;

	fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str("either `UTC_FORMAT_STR` or `NAIVE_FORMAT_STR`")
	}

	fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
	where
		E: de::Error,
	{
		DateTime::parse_from_str(v, UTC_FORMAT_STR).map_or_else(
			|_| {
				NaiveDateTime::parse_from_str(v, NAIVE_FORMAT_STR).map_or_else(
					|_| Err(E::custom("unable to parse utc or naive from str")),
					|time| Ok(Self::Value::Naive(time)),
				)
			},
			|time| Ok(Self::Value::Utc(time)),
		)
	}
}

impl<'de> Deserialize<'de> for MediaDate {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		deserializer.deserialize_str(MediaDateVisitor)
	}
}
