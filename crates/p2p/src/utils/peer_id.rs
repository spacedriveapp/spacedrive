use std::{fmt::Display, str::FromStr};

#[derive(Debug, Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(any(feature = "specta", feature = "serde"), serde(transparent))]
pub struct PeerId(
	#[cfg_attr(any(feature = "specta", feature = "serde"), specta(type = String))]
	pub(crate)  libp2p::PeerId,
);

// impl PeerId {
// 	pub fn to_string(&self) -> String {
// 		self.0.to_string()
// 	}
// }

impl FromStr for PeerId {
	#[allow(deprecated)]
	type Err = libp2p::core::ParseError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(Self(libp2p::PeerId::from_str(s)?))
	}
}

impl Display for PeerId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}
