use libp2p::kad::{store::MemoryStore, Behaviour as KadBehaviour, Config as KadConfig};
use libp2p::request_response::{
    Behaviour as RequestResponseBehaviour, Config as RequestResponseConfig, ProtocolSupport,
};
use libp2p::swarm::NetworkBehaviour;
use libp2p::{PeerId, StreamProtocol};
use libp2p::mdns;

use super::codec::PairingCodec;

/// Production NetworkBehaviour that combines Kademlia, mDNS, and request-response for pairing
#[derive(NetworkBehaviour)]
pub struct SpacedriveBehaviour {
    pub kademlia: KadBehaviour<MemoryStore>,
    pub request_response: RequestResponseBehaviour<PairingCodec>,
    pub mdns: mdns::tokio::Behaviour,
}

impl SpacedriveBehaviour {
    pub fn new(peer_id: PeerId) -> Result<Self, Box<dyn std::error::Error>> {
        // Create Kademlia behavior with memory store
        let store = MemoryStore::new(peer_id);
        let kad_config = KadConfig::default();
        let kademlia = KadBehaviour::with_config(peer_id, store, kad_config);

        // Create request-response behavior with simple codec
        let protocols = std::iter::once((
            StreamProtocol::new("/spacedrive/pairing/1.0.0"),
            ProtocolSupport::Full,
        ));
        let cfg = RequestResponseConfig::default();
        let request_response = RequestResponseBehaviour::with_codec(PairingCodec::default(), protocols, cfg);

        // Create mDNS behavior for local discovery
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)?;

        Ok(Self {
            kademlia,
            request_response,
            mdns,
        })
    }
}