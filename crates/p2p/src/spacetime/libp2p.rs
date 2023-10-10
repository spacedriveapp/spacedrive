//! This file contains of stuff to make libp2p work for us. They are fairly meaningless.

#[derive(Clone)]
pub struct SpaceTimeProtocolName(pub String);

impl AsRef<str> for SpaceTimeProtocolName {
	fn as_ref(&self) -> &str {
		&self.0
	}
}
