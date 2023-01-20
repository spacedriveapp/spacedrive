use std::{fmt::Display, str::FromStr};

#[derive(Debug, Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct PeerId(pub(crate) libp2p::PeerId);

impl FromStr for PeerId {
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

#[cfg(feature = "specta")]
impl specta::Type for PeerId {
	const NAME: &'static str = "PeerId";

	fn inline(opts: specta::DefOpts, generics: &[specta::DataType]) -> specta::DataType {
		<String as specta::Type>::inline(opts, generics)
	}

	fn reference(opts: specta::DefOpts, generics: &[specta::DataType]) -> specta::DataType {
		<String as specta::Type>::reference(opts, generics)
	}

	fn definition(opts: specta::DefOpts) -> specta::DataType {
		<String as specta::Type>::definition(opts)
	}
}
