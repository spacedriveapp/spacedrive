//! This file contains some fairly meaningless glue code for integrating with libp2p.

#[derive(Clone)]
pub struct SpaceTimeProtocolName(pub &'static str);

impl AsRef<str> for SpaceTimeProtocolName {
	fn as_ref(&self) -> &str {
		&self.0
	}
}
