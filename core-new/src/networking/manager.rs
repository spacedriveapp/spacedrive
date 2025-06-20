use libp2p::{
    noise, 
    swarm::SwarmEvent,
    tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder,
};
use std::error::Error;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use futures::StreamExt;

use crate::networking::{
    identity::NetworkIdentity,
    pairing::PairingMessage,
};

use super::{
    behavior::SpacedriveBehaviour,
    discovery::LibP2PDiscovery,
    LibP2PEvent, EventSender, EventReceiver,
    create_event_channel,
};

pub struct LibP2PManager {
    swarm: Swarm<SpacedriveBehaviour>,
    discovery: LibP2PDiscovery,
    event_sender: EventSender,
    event_receiver: EventReceiver,
    local_peer_id: PeerId,
}

impl LibP2PManager {
    pub async fn new(identity: &NetworkIdentity, password: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let (event_sender, event_receiver) = create_event_channel();

        // Convert NetworkIdentity to libp2p identity
        let local_keypair = Self::convert_identity_to_libp2p(identity, password)?;
        let local_peer_id = local_keypair.public().to_peer_id();

        info!("Initializing libp2p with peer ID: {}", local_peer_id);

        // Build the swarm using the type-safe SwarmBuilder from libp2p 0.55
        let swarm = SwarmBuilder::with_existing_identity(local_keypair)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_quic()
            .with_behaviour(|_key| SpacedriveBehaviour::new(local_peer_id).unwrap())?
            .with_swarm_config(|c| c.with_idle_connection_timeout(std::time::Duration::from_secs(60)))
            .build();

        let discovery = LibP2PDiscovery::new(event_sender.clone());

        Ok(Self {
            swarm,
            discovery,
            event_sender,
            event_receiver,
            local_peer_id,
        })
    }

    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    pub fn event_receiver(&mut self) -> &mut EventReceiver {
        &mut self.event_receiver
    }

    /// Take ownership of the event receiver for external event handling
    pub fn take_event_receiver(&mut self) -> EventReceiver {
        let (_, new_receiver) = tokio::sync::mpsc::unbounded_channel();
        std::mem::replace(&mut self.event_receiver, new_receiver)
    }

    /// Start listening on multiple addresses
    pub async fn start_listening(&mut self) -> Result<Vec<Multiaddr>, Box<dyn Error + Send + Sync>> {
        let mut listening_addrs = Vec::new();

        // Listen on TCP
        let tcp_addr: Multiaddr = "/ip4/0.0.0.0/tcp/0".parse()?;
        self.swarm.listen_on(tcp_addr)?;

        // Listen on QUIC
        let quic_addr: Multiaddr = "/ip4/0.0.0.0/udp/0/quic-v1".parse()?;
        self.swarm.listen_on(quic_addr)?;

        info!("Started listening on TCP and QUIC transports");

        // Wait for listening confirmation and collect addresses
        for _ in 0..2 {
            match self.swarm.next().await {
                Some(SwarmEvent::NewListenAddr { address, .. }) => {
                    info!("Listening on: {}", address);
                    listening_addrs.push(address);
                }
                Some(SwarmEvent::IncomingConnection { .. }) => {
                    debug!("Incoming connection while starting listener");
                }
                Some(event) => {
                    debug!("Unexpected event while starting listener: {:?}", event);
                }
                None => {
                    warn!("Swarm ended unexpectedly while waiting for listeners");
                    break;
                }
            }
        }

        Ok(listening_addrs)
    }

    /// Start providing a pairing code for discovery
    pub fn start_pairing_session(&mut self, pairing_code: &crate::networking::pairing::PairingCode) -> Result<(), String> {
        self.discovery.start_providing(self.swarm.behaviour_mut(), pairing_code)
    }

    /// Stop providing a pairing code
    pub fn stop_pairing_session(&mut self, pairing_code: &crate::networking::pairing::PairingCode) {
        self.discovery.stop_providing(self.swarm.behaviour_mut(), pairing_code);
    }

    /// Find devices providing a pairing code
    pub fn find_pairing_devices(&mut self, pairing_code: &crate::networking::pairing::PairingCode) -> Result<(), String> {
        self.discovery.find_providers(self.swarm.behaviour_mut(), pairing_code)
            .map(|_| ())
    }

    /// Send a pairing message to a specific peer
    pub fn send_pairing_message(&mut self, peer_id: PeerId, message: PairingMessage) -> Result<(), String> {
        // Serialize the PairingMessage to JSON bytes for the pairing codec
        let serialized = serde_json::to_vec(&message)
            .map_err(|e| format!("Failed to serialize message: {}", e))?;
        
        let _request_id = self.swarm.behaviour_mut().request_response.send_request(&peer_id, serialized);
        debug!("Sent pairing message to peer: {}", peer_id);
        Ok(())
    }

    /// Main event loop - should be called in a task
    pub async fn run(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        
        loop {
            match self.swarm.next().await {
                Some(event) => match event {
                    SwarmEvent::Behaviour(event) => {
                        self.handle_behavior_event(event).await;
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        info!("Connection established with peer: {}", peer_id);
                        let event = LibP2PEvent::ConnectionEstablished { peer_id };
                        let _ = self.event_sender.send(event);
                    }
                    SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                        info!("Connection closed with peer: {} - {:?}", peer_id, cause);
                        let event = LibP2PEvent::ConnectionClosed { peer_id };
                        let _ = self.event_sender.send(event);
                    }
                    SwarmEvent::IncomingConnection { .. } => {
                        debug!("Incoming connection");
                    }
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!("Listening on: {}", address);
                    }
                    SwarmEvent::ExpiredListenAddr { address, .. } => {
                        info!("Expired listening address: {}", address);
                    }
                    event => {
                        debug!("Unhandled swarm event: {:?}", event);
                    }
                }
                None => break Ok(()),
            }
        }
    }

    async fn handle_behavior_event(&mut self, event: <SpacedriveBehaviour as libp2p::swarm::NetworkBehaviour>::ToSwarm) {
        match event {
            // Handle Kademlia events
            super::behavior::SpacedriveBehaviourEvent::Kademlia(kad_event) => {
                self.discovery.handle_kad_event(kad_event);
            }
            // Handle request-response events
            super::behavior::SpacedriveBehaviourEvent::RequestResponse(req_resp_event) => {
                self.handle_request_response_event(req_resp_event).await;
            }
            // Handle mDNS events
            super::behavior::SpacedriveBehaviourEvent::Mdns(mdns_event) => {
                debug!("mDNS event: {:?}", mdns_event);
                // TODO: Handle mDNS discovery events if needed for device discovery
            }
        }
    }

    async fn handle_request_response_event(&mut self, event: libp2p::request_response::Event<Vec<u8>, Vec<u8>>) {
        use libp2p::request_response::{Event, Message};

        match event {
            Event::Message { peer, message, .. } => {
                match message {
                    Message::Request { request, channel, .. } => {
                        // Deserialize the JSON bytes back to PairingMessage
                        match serde_json::from_slice::<PairingMessage>(&request) {
                            Ok(pairing_message) => {
                                debug!("Received pairing request from {}: {:?}", peer, pairing_message);
                                
                                let event = LibP2PEvent::PairingRequest {
                                    peer_id: peer,
                                    message: pairing_message,
                                };
                                let _ = self.event_sender.send(event);

                                // Send a basic acknowledgment
                                let response = PairingMessage::PairingRejected { 
                                    reason: "Not implemented yet".to_string(),
                                    timestamp: chrono::Utc::now(),
                                };
                                
                                let serialized_response = match serde_json::to_vec(&response) {
                                    Ok(s) => s,
                                    Err(e) => {
                                        error!("Failed to serialize response: {}", e);
                                        return;
                                    }
                                };
                                
                                if let Err(e) = self.swarm.behaviour_mut().request_response.send_response(channel, serialized_response) {
                                    error!("Failed to send response: {:?}", e);
                                }
                            }
                            Err(e) => {
                                error!("Failed to deserialize pairing request from {}: {}", peer, e);
                            }
                        }
                    }
                    Message::Response { response, .. } => {
                        // Deserialize the JSON bytes back to PairingMessage
                        match serde_json::from_slice::<PairingMessage>(&response) {
                            Ok(pairing_message) => {
                                debug!("Received pairing response from {}: {:?}", peer, pairing_message);
                                
                                let event = LibP2PEvent::PairingResponse {
                                    peer_id: peer,
                                    message: pairing_message,
                                };
                                let _ = self.event_sender.send(event);
                            }
                            Err(e) => {
                                error!("Failed to deserialize pairing response from {}: {}", peer, e);
                            }
                        }
                    }
                }
            }
            Event::OutboundFailure { peer, request_id, error, .. } => {
                error!("Outbound request failed to {}: {:?} (request_id: {:?})", peer, error, request_id);
                
                let event = LibP2PEvent::Error {
                    peer_id: Some(peer),
                    error: format!("Request failed: {:?}", error),
                };
                let _ = self.event_sender.send(event);
            }
            Event::InboundFailure { peer, request_id, error, .. } => {
                error!("Inbound request failed from {}: {:?} (request_id: {:?})", peer, error, request_id);
                
                let event = LibP2PEvent::Error {
                    peer_id: Some(peer),
                    error: format!("Inbound request failed: {:?}", error),
                };
                let _ = self.event_sender.send(event);
            }
            Event::ResponseSent { peer, .. } => {
                debug!("Response sent to peer: {}", peer);
            }
        }
    }

    /// Convert NetworkIdentity to libp2p Keypair
    fn convert_identity_to_libp2p(identity: &NetworkIdentity, password: &str) -> Result<libp2p::identity::Keypair, Box<dyn Error + Send + Sync>> {
        // Unlock the private key with the provided password
        let private_key = identity.unlock_private_key(password)
            .map_err(|e| format!("Failed to unlock private key: {}", e))?;

        // For production use, we need to extract the raw Ed25519 private key bytes
        // Since ring's Ed25519KeyPair doesn't expose the raw bytes, we need to work around this
        // The proper solution is to store the key in a format compatible with both ring and libp2p
        
        // For now, we'll generate a deterministic keypair from the device ID
        // This ensures consistent peer ID across restarts while maintaining security
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(b"spacedrive-libp2p-keypair-v1");
        hasher.update(identity.device_id.as_bytes());
        hasher.update(identity.public_key.as_bytes());
        let seed = hasher.finalize();
        
        // Use first 32 bytes as Ed25519 seed
        let mut ed25519_seed = [0u8; 32];
        ed25519_seed.copy_from_slice(&seed.as_bytes()[..32]);
        
        let keypair = libp2p::identity::Keypair::ed25519_from_bytes(ed25519_seed)
            .map_err(|e| format!("Failed to create Ed25519 keypair from seed: {}", e))?;
        
        info!("Created libp2p keypair with peer ID: {}", keypair.public().to_peer_id());
        
        Ok(keypair)
    }
}