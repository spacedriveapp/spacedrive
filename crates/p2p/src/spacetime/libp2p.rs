//! This file contains of stuff to make libp2p work for us. They are fairly meaningless.

#[derive(Clone)]
pub struct SpaceTimeProtocolName(pub &'static [u8]);

impl libp2p::core::ProtocolName for SpaceTimeProtocolName {
	fn protocol_name(&self) -> &[u8] {
		self.0
	}
}
