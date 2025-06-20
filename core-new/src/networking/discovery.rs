use libp2p::{
    kad::{QueryId, QueryResult},
    PeerId,
};
use std::collections::HashMap;
use tracing::{debug, error, info};

use crate::networking::pairing::PairingCode;
use super::{SpacedriveBehaviour, LibP2PEvent, EventSender};

pub struct LibP2PDiscovery {
    active_queries: HashMap<QueryId, PairingCode>,
    event_sender: EventSender,
}

impl LibP2PDiscovery {
    pub fn new(event_sender: EventSender) -> Self {
        Self {
            active_queries: HashMap::new(),
            event_sender,
        }
    }

    /// Start providing a pairing code on the DHT
    pub fn start_providing(&mut self, behavior: &mut SpacedriveBehaviour, code: &PairingCode) -> Result<(), String> {
        let key = libp2p::kad::RecordKey::new(&code.discovery_fingerprint);
        
        debug!("Starting to provide pairing code: {}", code.as_words());
        
        match behavior.kademlia.start_providing(key.clone()) {
            Ok(query_id) => {
                self.active_queries.insert(query_id, code.clone());
                info!("Started providing pairing code with query ID: {:?}", query_id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to start providing pairing code: {:?}", e);
                Err(format!("Failed to start providing: {:?}", e))
            }
        }
    }

    /// Stop providing a pairing code on the DHT
    pub fn stop_providing(&mut self, behavior: &mut SpacedriveBehaviour, code: &PairingCode) {
        let key = libp2p::kad::RecordKey::new(&code.discovery_fingerprint);
        behavior.kademlia.stop_providing(&key);
        
        debug!("Stopped providing pairing code: {}", code.as_words());
    }

    /// Find providers for a pairing code
    pub fn find_providers(&mut self, behavior: &mut SpacedriveBehaviour, code: &PairingCode) -> Result<QueryId, String> {
        let key = libp2p::kad::RecordKey::new(&code.discovery_fingerprint);
        
        debug!("Finding providers for pairing code: {}", code.as_words());
        
        let query_id = behavior.kademlia.get_providers(key);
        self.active_queries.insert(query_id, code.clone());
        
        Ok(query_id)
    }

    /// Handle Kademlia events
    pub fn handle_kad_event(&mut self, event: libp2p::kad::Event) {
        match event {
            libp2p::kad::Event::OutboundQueryProgressed {
                id,
                result: QueryResult::GetProviders(result),
                ..
            } => {
                self.handle_get_providers_result(id, result);
            }
            libp2p::kad::Event::OutboundQueryProgressed {
                id,
                result: QueryResult::StartProviding(result),
                ..
            } => {
                self.handle_start_providing_result(id, result);
            }
            libp2p::kad::Event::InboundRequest { .. } => {
                // Handle inbound DHT requests if needed
                debug!("Received inbound DHT request");
            }
            _ => {
                // Handle other Kademlia events as needed
                debug!("Received other Kademlia event: {:?}", event);
            }
        }
    }

    fn handle_get_providers_result(&mut self, query_id: QueryId, result: Result<libp2p::kad::GetProvidersOk, libp2p::kad::GetProvidersError>) {
        if let Some(pairing_code) = self.active_queries.remove(&query_id) {
            match result {
                Ok(libp2p::kad::GetProvidersOk::FoundProviders { providers, .. }) => {
                    info!("Found {} providers for pairing code: {}", providers.len(), "redacted");
                    
                    for peer_id in providers {
                        // Emit discovery event
                        let event = LibP2PEvent::DeviceDiscovered {
                            peer_id,
                            addr: format!("/p2p/{}", peer_id).parse().unwrap_or_else(|_| {
                                // Fallback multiaddr
                                format!("/ip4/127.0.0.1/tcp/0/p2p/{}", peer_id).parse().unwrap()
                            })
                        };
                        
                        if let Err(e) = self.event_sender.send(event) {
                            error!("Failed to send discovery event: {}", e);
                        }
                    }
                }
                Ok(libp2p::kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. }) => {
                    info!("No additional providers found for pairing code");
                }
                Err(e) => {
                    error!("Failed to get providers for pairing code: {:?}", e);
                    
                    let event = LibP2PEvent::Error {
                        peer_id: None,
                        error: format!("Discovery failed: {:?}", e),
                    };
                    
                    if let Err(e) = self.event_sender.send(event) {
                        error!("Failed to send error event: {}", e);
                    }
                }
            }
        }
    }

    fn handle_start_providing_result(&mut self, query_id: QueryId, result: Result<libp2p::kad::AddProviderOk, libp2p::kad::AddProviderError>) {
        if let Some(pairing_code) = self.active_queries.remove(&query_id) {
            match result {
                Ok(_) => {
                    info!("Successfully started providing pairing code");
                }
                Err(e) => {
                    error!("Failed to start providing pairing code: {:?}", e);
                    
                    let event = LibP2PEvent::Error {
                        peer_id: None,
                        error: format!("Failed to start providing: {:?}", e),
                    };
                    
                    if let Err(e) = self.event_sender.send(event) {
                        error!("Failed to send error event: {}", e);
                    }
                }
            }
        }
    }
}