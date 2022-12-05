// TODO: Ban clients, timeout malicious clients, etc

use rustls::Certificate;

use crate::PeerId;

pub struct Protector {
    // // Store the list of banned peers in an eventually consistent map
    // banned: DashSet<PeerId>, // TODO: Use strongly consistent data structure
}

impl Protector {
    pub(crate) fn new() -> Self {
        Self {
            // banned: Default::default(),
        }
    }

    pub(crate) fn on_cert_verify(&self, cert: &Certificate) -> bool {
        // let peer_id = PeerId::from_cert(cert);
        // return !self.banned.contains(&peer_id);
        return true; // TODO
    }

    /// TODO
    pub fn ban_peer(&self, peer_id: PeerId) {
        // self.banned.insert(peer_id); // TODO

        // TODO: Remove active connections
    }
}
