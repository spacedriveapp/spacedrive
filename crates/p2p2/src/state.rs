use std::sync::Arc;

use rustls::Certificate;

use crate::{services::Protector, PeerId};

/// TODO
pub struct State {
    // TODO: pub discovery: Discovery,
    pub protector: Protector,
}

impl State {
    /// construct a new instance of `State`.
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            protector: Protector::new(),
        })
    }

    /// is a hook which is trigger during the TLS handshake to check if the connection should proceed.
    pub(crate) fn on_cert_verify(&self, cert: &Certificate) -> bool {
        return self.protector.on_cert_verify(cert);
    }

    /// is a hook which is trigger during the establishment of a new incoming connection.
    pub(crate) fn on_incoming_connection(&self, peer_id: &PeerId) -> bool {
        return true; // TODO
    }

    /// is a hook which is trigger during the establishment of a new stream on an existing connection.
    pub(crate) fn on_incoming_stream(&self, peer_id: &PeerId) -> bool {
        return true; // TODO
    }
}
