use crate::P2P;

pub struct Mdns {}

impl Mdns {
	pub fn attach(p2p: &P2P) -> Self {
		Mdns {}
	}

	pub fn deattach(self) {
		// TODO: Deregister this
	}
}

// pub addresses: Vec<SocketAddr>,
