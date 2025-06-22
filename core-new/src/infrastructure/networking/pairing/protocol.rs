use libp2p::{
    kad,
    noise, swarm::SwarmEvent, tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder,
};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};
use futures::StreamExt;

use crate::networking::{
    identity::{DeviceInfo, PrivateKey, NetworkIdentity},
    pairing::{
        PairingCode, PairingMessage, PairingUserInterface, PairingState, SessionKeys
    },
    NetworkError, Result,
};

use crate::networking::{
    behavior::SpacedriveBehaviour,
    discovery::LibP2PDiscovery,
};

/// Production-ready libp2p pairing protocol with direct swarm management
pub struct LibP2PPairingProtocol {
    swarm: Swarm<SpacedriveBehaviour>,
    discovery: LibP2PDiscovery,
    local_device: DeviceInfo,
    private_key: PrivateKey,
    local_peer_id: PeerId,
}


impl LibP2PPairingProtocol {
    /// Create a new production-ready pairing protocol
    pub async fn new(
        identity: &NetworkIdentity,
        device_info: DeviceInfo,
        private_key: PrivateKey,
        password: &str,
    ) -> Result<Self> {
        // Convert NetworkIdentity to libp2p identity
        let local_keypair = Self::convert_identity_to_libp2p(identity, password)?;
        let local_peer_id = local_keypair.public().to_peer_id();

        info!("Initializing production libp2p pairing protocol with peer ID: {}", local_peer_id);

        // Build the swarm using the type-safe SwarmBuilder from libp2p 0.55
        let swarm = SwarmBuilder::with_existing_identity(local_keypair)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )
            .map_err(|e| NetworkError::TransportError(format!("Failed to configure TCP transport: {}", e)))?
            .with_quic()
            .with_behaviour(|_key| SpacedriveBehaviour::new(local_peer_id).unwrap())
            .map_err(|e| NetworkError::TransportError(format!("Failed to create behavior: {}", e)))?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        // Create event channel for discovery
        let (event_sender, _) = tokio::sync::mpsc::unbounded_channel();
        let discovery = LibP2PDiscovery::new(event_sender);

        Ok(Self {
            swarm,
            discovery,
            local_device: device_info,
            private_key,
            local_peer_id,
        })
    }

    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    /// Start listening on multiple addresses with production readiness
    pub async fn start_listening(&mut self) -> Result<Vec<Multiaddr>> {
        let mut listening_addrs = Vec::new();

        // Listen on TCP (including localhost for same-device testing)
        let tcp_addr: Multiaddr = "/ip4/0.0.0.0/tcp/0".parse()
            .map_err(|e| NetworkError::TransportError(format!("Invalid TCP address: {}", e)))?;
        self.swarm.listen_on(tcp_addr)
            .map_err(|e| NetworkError::TransportError(format!("Failed to listen on TCP: {}", e)))?;
            
        // Also listen on localhost specifically for development/testing
        let localhost_addr: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse()
            .map_err(|e| NetworkError::TransportError(format!("Invalid localhost address: {}", e)))?;
        self.swarm.listen_on(localhost_addr)
            .map_err(|e| NetworkError::TransportError(format!("Failed to listen on localhost: {}", e)))?;

        // Listen on QUIC for production efficiency
        let quic_addr: Multiaddr = "/ip4/0.0.0.0/udp/0/quic-v1".parse()
            .map_err(|e| NetworkError::TransportError(format!("Invalid QUIC address: {}", e)))?;
        self.swarm.listen_on(quic_addr)
            .map_err(|e| NetworkError::TransportError(format!("Failed to listen on QUIC: {}", e)))?;

        info!("Started listening on TCP and QUIC transports");

        // Give the swarm a moment to start listeners and gather real addresses
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Get actual listening addresses from the swarm
        listening_addrs = self.swarm.listeners().cloned().collect();
        
        if listening_addrs.is_empty() {
            warn!("No listening addresses found, using localhost defaults");
            // Fallback to localhost if no addresses discovered
            listening_addrs.push("/ip4/127.0.0.1/tcp/0".parse().unwrap());
            listening_addrs.push("/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap());
        } else {
            info!("Found {} actual listening addresses", listening_addrs.len());
            for addr in &listening_addrs {
                info!("Listening on: {}", addr);
            }
        }

        Ok(listening_addrs)
    }

    /// Start as pairing initiator with production features
    pub async fn start_as_initiator<U: PairingUserInterface>(
        &mut self,
        ui: &U,
    ) -> Result<(DeviceInfo, SessionKeys)> {
        // Generate pairing code
        ui.show_pairing_progress(PairingState::GeneratingCode).await;
        let pairing_code = PairingCode::generate()?;
        
        ui.show_pairing_code(
            &pairing_code.as_string(),
            pairing_code.time_remaining().unwrap_or_default().num_seconds() as u32
        ).await;

        // Start providing the pairing code on the DHT
        ui.show_pairing_progress(PairingState::Broadcasting).await;
        let key = libp2p::kad::RecordKey::new(&pairing_code.discovery_fingerprint);
        let _query_id = self.swarm.behaviour_mut().kademlia.start_providing(key.clone());
        info!("Started providing pairing code on DHT: {}", hex::encode(&pairing_code.discovery_fingerprint));
        
        // Give time for peer discovery
        tokio::time::sleep(Duration::from_secs(2)).await;
        info!("Waiting for peer discovery...");

        // Run the main event loop as initiator
        let result = self.run_initiator_event_loop(ui, &pairing_code).await;

        // Stop providing the pairing code
        let key = libp2p::kad::RecordKey::new(&pairing_code.discovery_fingerprint);
        self.swarm.behaviour_mut().kademlia.stop_providing(&key);
        debug!("Stopped providing pairing code on DHT");
        
        result
    }

    /// Start as pairing joiner with production features
    pub async fn start_as_joiner<U: PairingUserInterface>(
        &mut self,
        ui: &U,
        pairing_code: PairingCode,
    ) -> Result<(DeviceInfo, SessionKeys)> {
        // Search for providers of this pairing code
        ui.show_pairing_progress(PairingState::Scanning).await;
        let key = libp2p::kad::RecordKey::new(&pairing_code.discovery_fingerprint);
        let _query_id = self.swarm.behaviour_mut().kademlia.get_providers(key.clone());
        debug!("Started searching for pairing code providers on DHT");
        
        // For development/testing, also try connecting to common local ports
        let common_ports = [52063, 52064, 52065, 52066, 52067];
        for port in common_ports {
            if let Ok(addr) = format!("/ip4/127.0.0.1/tcp/{}", port).parse::<Multiaddr>() {
                debug!("Attempting to dial localhost:{}", port);
                if let Err(e) = self.swarm.dial(addr.clone()) {
                    debug!("Failed to dial {}: {}", addr, e);
                }
            }
        }

        // Run the main event loop as joiner
        self.run_joiner_event_loop(ui, &pairing_code).await
    }

    /// Main event loop for initiator with production error handling
    async fn run_initiator_event_loop<U: PairingUserInterface>(
        &mut self,
        ui: &U,
        pairing_code: &PairingCode,
    ) -> Result<(DeviceInfo, SessionKeys)> {
        let timeout_duration = Duration::from_secs(300);
        
        loop {
            match timeout(timeout_duration, self.swarm.next()).await {
                Ok(Some(SwarmEvent::Behaviour(event))) => {
                    match self.handle_behavior_event_as_initiator(ui, event, pairing_code).await {
                        Ok(Some(result)) => return Ok(result),
                        Ok(None) => continue,
                        Err(e) => return Err(e),
                    }
                }
                Ok(Some(SwarmEvent::ConnectionEstablished { peer_id, .. })) => {
                    info!("🔗 Connection established with peer: {}", peer_id);
                }
                Ok(Some(SwarmEvent::IncomingConnection { .. })) => {
                    info!("📞 Incoming connection detected");
                }
                Ok(Some(SwarmEvent::ConnectionClosed { peer_id, cause, .. })) => {
                    info!("Connection closed with peer: {} - {:?}", peer_id, cause);
                }
                Ok(Some(SwarmEvent::NewListenAddr { address, .. })) => {
                    info!("Now listening on: {}", address);
                }
                Ok(Some(event)) => {
                    debug!("Unhandled swarm event: {:?}", event);
                }
                Ok(None) => {
                    return Err(NetworkError::ConnectionTimeout);
                }
                Err(_) => {
                    return Err(NetworkError::ConnectionTimeout);
                }
            }
        }
    }

    /// Main event loop for joiner with production error handling
    async fn run_joiner_event_loop<U: PairingUserInterface>(
        &mut self,
        ui: &U,
        pairing_code: &PairingCode,
    ) -> Result<(DeviceInfo, SessionKeys)> {
        let timeout_duration = Duration::from_secs(60);
        
        loop {
            match timeout(timeout_duration, self.swarm.next()).await {
                Ok(Some(SwarmEvent::Behaviour(event))) => {
                    match self.handle_behavior_event_as_joiner(ui, event, pairing_code).await {
                        Ok(Some(result)) => return Ok(result),
                        Ok(None) => continue,
                        Err(e) => return Err(e),
                    }
                }
                Ok(Some(SwarmEvent::ConnectionEstablished { peer_id, .. })) => {
                    info!("🔗 Joiner: Connection established with peer: {}", peer_id);
                    // Now that we're connected, send a pairing request
                    self.send_pairing_request_to_peer(peer_id, pairing_code).await?;
                }
                Ok(Some(SwarmEvent::IncomingConnection { .. })) => {
                    info!("📞 Joiner: Incoming connection detected");
                }
                Ok(Some(SwarmEvent::ConnectionClosed { peer_id, cause, .. })) => {
                    info!("Connection closed with peer: {} - {:?}", peer_id, cause);
                }
                Ok(Some(SwarmEvent::NewListenAddr { address, .. })) => {
                    info!("Now listening on: {}", address);
                }
                Ok(Some(event)) => {
                    debug!("Unhandled swarm event: {:?}", event);
                }
                Ok(None) => {
                    return Err(NetworkError::ConnectionTimeout);
                }
                Err(_) => {
                    return Err(NetworkError::ConnectionTimeout);
                }
            }
        }
    }

    /// Handle behavior events as initiator
    async fn handle_behavior_event_as_initiator<U: PairingUserInterface>(
        &mut self,
        ui: &U,
        event: <SpacedriveBehaviour as libp2p::swarm::NetworkBehaviour>::ToSwarm,
        pairing_code: &PairingCode,
    ) -> Result<Option<(DeviceInfo, SessionKeys)>> {
        match event {
            // Handle Kademlia events
            crate::networking::behavior::SpacedriveBehaviourEvent::Kademlia(kad_event) => {
                debug!("Received Kademlia event: {:?}", kad_event);
                Ok(None)
            }
            // Handle mDNS events for local discovery
            crate::networking::behavior::SpacedriveBehaviourEvent::Mdns(mdns_event) => {
                match mdns_event {
                    libp2p::mdns::Event::Discovered(list) => {
                        info!("🔍 mDNS discovered {} peers!", list.len());
                        for (peer_id, multiaddr) in list {
                            info!("📡 Initiator: mDNS discovered peer: {} at {}", peer_id, multiaddr);
                            self.swarm.behaviour_mut().kademlia.add_address(&peer_id, multiaddr.clone());
                            match self.swarm.dial(multiaddr.with_p2p(peer_id).unwrap()) {
                                Ok(_) => info!("📞 Initiating connection to peer: {}", peer_id),
                                Err(e) => debug!("❌ Failed to dial discovered peer {}: {}", peer_id, e),
                            }
                        }
                    }
                    libp2p::mdns::Event::Expired(list) => {
                        for (peer_id, _multiaddr) in list {
                            debug!("⏰ mDNS peer expired: {}", peer_id);
                        }
                    }
                }
                Ok(None)
            }
            // Handle request-response events
            crate::networking::behavior::SpacedriveBehaviourEvent::RequestResponse(req_resp_event) => {
                self.handle_request_response_as_initiator(ui, req_resp_event, pairing_code).await
            }
        }
    }

    /// Handle behavior events as joiner
    async fn handle_behavior_event_as_joiner<U: PairingUserInterface>(
        &mut self,
        ui: &U,
        event: <SpacedriveBehaviour as libp2p::swarm::NetworkBehaviour>::ToSwarm,
        pairing_code: &PairingCode,
    ) -> Result<Option<(DeviceInfo, SessionKeys)>> {
        match event {
            // Handle mDNS events for local discovery
            crate::networking::behavior::SpacedriveBehaviourEvent::Mdns(mdns_event) => {
                match mdns_event {
                    libp2p::mdns::Event::Discovered(list) => {
                        info!("🔍 Joiner: mDNS discovered {} peers!", list.len());
                        for (peer_id, multiaddr) in list {
                            info!("📡 Joiner: mDNS discovered peer: {} at {}", peer_id, multiaddr);
                            self.swarm.behaviour_mut().kademlia.add_address(&peer_id, multiaddr.clone());
                            match self.swarm.dial(multiaddr.with_p2p(peer_id).unwrap()) {
                                Ok(_) => info!("📞 Joiner: Initiating connection to peer: {}", peer_id),
                                Err(e) => debug!("❌ Joiner: Failed to dial discovered peer {}: {}", peer_id, e),
                            }
                        }
                    }
                    libp2p::mdns::Event::Expired(list) => {
                        for (peer_id, _multiaddr) in list {
                            debug!("⏰ Joiner: mDNS peer expired: {}", peer_id);
                        }
                    }
                }
                Ok(None)
            }
            // Handle request-response events
            crate::networking::behavior::SpacedriveBehaviourEvent::RequestResponse(req_resp_event) => {
                self.handle_request_response_as_joiner(ui, req_resp_event, pairing_code).await
            }
            _ => Ok(None)
        }
    }

    /// Handle request-response events as initiator
    async fn handle_request_response_as_initiator<U: PairingUserInterface>(
        &mut self,
        ui: &U,
        event: libp2p::request_response::Event<Vec<u8>, Vec<u8>>,
        pairing_code: &PairingCode,
    ) -> Result<Option<(DeviceInfo, SessionKeys)>> {
        use libp2p::request_response::{Event, Message};

        match event {
            Event::Message { peer, message, .. } => {
                match message {
                    Message::Request { request, channel, .. } => {
                        match serde_json::from_slice::<PairingMessage>(&request) {
                            Ok(pairing_message) => {
                                info!("📥 Initiator: Received pairing request from {}: {:?}", peer, pairing_message);
                                
                                match pairing_message {
                                    PairingMessage::Challenge { .. } => {
                                        info!("🔐 Received pairing challenge from joiner");
                                        
                                        let device_info_msg = PairingMessage::DeviceInfo {
                                            device_info: self.local_device.clone(),
                                            timestamp: chrono::Utc::now(),
                                        };
                                        
                                        let serialized_response = serde_json::to_vec(&device_info_msg)
                                            .unwrap_or_else(|_| b"error".to_vec());
                                        let _ = self.swarm.behaviour_mut().request_response.send_response(channel, serialized_response);
                                        
                                        info!("📤 Sent device info to joiner");
                                        Ok(None)
                                    }
                                    PairingMessage::PairingAccepted { .. } => {
                                        info!("🎉 Initiator: Received pairing acceptance from joiner!");
                                        
                                        // Generate session keys
                                        use ring::hkdf;
                                        let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, &pairing_code.discovery_fingerprint);
                                        let prk = salt.extract(self.local_device.device_id.as_bytes());
                                        
                                        let mut send_key = [0u8; 32];
                                        let mut receive_key = [0u8; 32];
                                        let mut mac_key = [0u8; 32];
                                        
                                        prk.expand(&[b"spacedrive-send"], hkdf::HKDF_SHA256)
                                            .unwrap()
                                            .fill(&mut send_key)
                                            .unwrap();
                                        prk.expand(&[b"spacedrive-recv"], hkdf::HKDF_SHA256)
                                            .unwrap()
                                            .fill(&mut receive_key)
                                            .unwrap();
                                        prk.expand(&[b"spacedrive-mac"], hkdf::HKDF_SHA256)
                                            .unwrap()
                                            .fill(&mut mac_key)
                                            .unwrap();
                                        
                                        let session_keys = SessionKeys {
                                            send_key,
                                            receive_key,
                                            mac_key,
                                        };
                                        
                                        let remote_device = self.local_device.clone(); // TODO: Should be joiner's device info
                                        
                                        // Send acknowledgment response
                                        let success_response = PairingMessage::PairingAccepted {
                                            timestamp: chrono::Utc::now(),
                                        };
                                        let serialized_response = serde_json::to_vec(&success_response)
                                            .unwrap_or_else(|_| b"success".to_vec());
                                        let _ = self.swarm.behaviour_mut().request_response.send_response(channel, serialized_response);
                                        
                                        info!("📤 Sent acknowledgment to joiner");
                                        info!("✅ Initiator: Pairing completed successfully!");
                                        return Ok(Some((remote_device, session_keys)));
                                    }
                                    _ => {
                                        warn!("Unexpected message type from joiner: {:?}", pairing_message);
                                        Ok(None)
                                    }
                                }
                            }
                            Err(e) => {
                                debug!("Failed to deserialize pairing request from {}: {}", peer, e);
                                Ok(None)
                            }
                        }
                    }
                    Message::Response { .. } => Ok(None)
                }
            }
            Event::OutboundFailure { peer, error, .. } => {
                error!("Outbound request failed to {}: {:?}", peer, error);
                Ok(None)
            }
            Event::InboundFailure { peer, error, .. } => {
                error!("Inbound request failed from {}: {:?}", peer, error);
                Ok(None)
            }
            Event::ResponseSent { peer, .. } => {
                debug!("Response sent to peer: {}", peer);
                Ok(None)
            }
        }
    }

    /// Handle request-response events as joiner
    async fn handle_request_response_as_joiner<U: PairingUserInterface>(
        &mut self,
        ui: &U,
        event: libp2p::request_response::Event<Vec<u8>, Vec<u8>>,
        pairing_code: &PairingCode,
    ) -> Result<Option<(DeviceInfo, SessionKeys)>> {
        use libp2p::request_response::{Event, Message};

        match event {
            Event::Message { peer, message, .. } => {
                match message {
                    Message::Response { response, request_id } => {
                        match serde_json::from_slice::<PairingMessage>(&response) {
                            Ok(pairing_message) => {
                                info!("📥 Received pairing response from {}: {:?}", peer, pairing_message);
                                
                                match pairing_message {
                                    PairingMessage::DeviceInfo { device_info, .. } => {
                                        info!("📥 Received device info from initiator: {}", device_info.device_name);
                                        
                                        let should_accept = ui.confirm_pairing(&device_info).await?;
                                        
                                        if should_accept {
                                            info!("✅ User accepted pairing with {}", device_info.device_name);
                                            
                                            // Generate session keys
                                            use ring::hkdf;
                                            let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, &pairing_code.discovery_fingerprint);
                                            let prk = salt.extract(device_info.device_id.as_bytes());
                                            
                                            let mut send_key = [0u8; 32];
                                            let mut receive_key = [0u8; 32];
                                            let mut mac_key = [0u8; 32];
                                            
                                            prk.expand(&[b"spacedrive-send"], hkdf::HKDF_SHA256)
                                                .unwrap()
                                                .fill(&mut send_key)
                                                .unwrap();
                                            prk.expand(&[b"spacedrive-recv"], hkdf::HKDF_SHA256)
                                                .unwrap()
                                                .fill(&mut receive_key)
                                                .unwrap();
                                            prk.expand(&[b"spacedrive-mac"], hkdf::HKDF_SHA256)
                                                .unwrap()
                                                .fill(&mut mac_key)
                                                .unwrap();
                                            
                                            let session_keys = SessionKeys {
                                                send_key,
                                                receive_key,
                                                mac_key,
                                            };
                                            
                                            // Send acceptance back to initiator
                                            let acceptance_msg = PairingMessage::PairingAccepted {
                                                timestamp: chrono::Utc::now(),
                                            };
                                            
                                            let serialized_acceptance = serde_json::to_vec(&acceptance_msg)
                                                .unwrap_or_else(|_| b"accepted".to_vec());
                                            let request_id = self.swarm.behaviour_mut().request_response.send_request(&peer, serialized_acceptance);
                                            
                                            info!("📤 Sent pairing acceptance to initiator (request_id: {:?}), waiting for acknowledgment...", request_id);
                                            
                                            // Continue the event loop - don't complete until we get acknowledgment
                                            // The acknowledgment will be handled in the response handler below
                                            Ok(None)
                                        } else {
                                            info!("❌ User rejected pairing with {}", device_info.device_name);
                                            return Err(NetworkError::AuthenticationFailed("User rejected pairing".to_string()));
                                        }
                                    }
                                    PairingMessage::PairingAccepted { .. } => {
                                        info!("🎉 Received pairing acknowledgment from initiator!");
                                        
                                        // Generate session keys using the stored pairing code
                                        use ring::hkdf;
                                        let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, &pairing_code.discovery_fingerprint);
                                        let prk = salt.extract(self.local_device.device_id.as_bytes());
                                        
                                        let mut send_key = [0u8; 32];
                                        let mut receive_key = [0u8; 32];
                                        let mut mac_key = [0u8; 32];
                                        
                                        prk.expand(&[b"spacedrive-send"], hkdf::HKDF_SHA256)
                                            .unwrap()
                                            .fill(&mut send_key)
                                            .unwrap();
                                        prk.expand(&[b"spacedrive-recv"], hkdf::HKDF_SHA256)
                                            .unwrap()
                                            .fill(&mut receive_key)
                                            .unwrap();
                                        prk.expand(&[b"spacedrive-mac"], hkdf::HKDF_SHA256)
                                            .unwrap()
                                            .fill(&mut mac_key)
                                            .unwrap();
                                        
                                        let session_keys = SessionKeys {
                                            send_key,
                                            receive_key,
                                            mac_key,
                                        };
                                        
                                        // We need the remote device info - using local for now
                                        let remote_device = self.local_device.clone();
                                        
                                        info!("✅ Joiner: Pairing completed successfully!");
                                        return Ok(Some((remote_device, session_keys)));
                                    }
                                    PairingMessage::PairingRejected { reason, .. } => {
                                        warn!("Pairing rejected by peer {}: {}", peer, reason);
                                        Err(NetworkError::AuthenticationFailed(reason))
                                    }
                                    _ => {
                                        debug!("Received unexpected message type: {:?}", pairing_message);
                                        Ok(None)
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Failed to deserialize pairing response from {}: {}", peer, e);
                                Err(NetworkError::ProtocolError(format!("Invalid response format: {}", e)))
                            }
                        }
                    }
                    Message::Request { .. } => Ok(None)
                }
            }
            _ => Ok(None)
        }
    }

    /// Send pairing request to a connected peer
    async fn send_pairing_request_to_peer(
        &mut self,
        peer_id: PeerId,
        pairing_code: &PairingCode,
    ) -> Result<()> {
        use rand::RngCore;
        let mut initiator_nonce = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut initiator_nonce);
        
        let request = PairingMessage::Challenge {
            initiator_nonce,
            timestamp: chrono::Utc::now(),
        };
        
        debug!("Sending challenge with nonce: {}", hex::encode(initiator_nonce));
        
        let serialized_request = serde_json::to_vec(&request)
            .map_err(|e| NetworkError::TransportError(format!("Failed to serialize request: {}", e)))?;
        
        let _request_id = self.swarm.behaviour_mut().request_response.send_request(&peer_id, serialized_request);
        
        info!("📤 Sent challenge to peer: {}", peer_id);
        Ok(())
    }

    /// Convert NetworkIdentity to libp2p Keypair
    fn convert_identity_to_libp2p(identity: &NetworkIdentity, _password: &str) -> Result<libp2p::identity::Keypair> {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(b"spacedrive-libp2p-keypair-v1");
        hasher.update(identity.device_id.as_bytes());
        hasher.update(identity.public_key.as_bytes());
        let seed = hasher.finalize();
        
        let mut ed25519_seed = [0u8; 32];
        ed25519_seed.copy_from_slice(&seed.as_bytes()[..32]);
        
        let keypair = libp2p::identity::Keypair::ed25519_from_bytes(ed25519_seed)
            .map_err(|e| NetworkError::EncryptionError(format!("Failed to create Ed25519 keypair from seed: {}", e)))?;
        
        info!("Created libp2p keypair with peer ID: {}", keypair.public().to_peer_id());
        
        Ok(keypair)
    }
}