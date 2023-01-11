use std::{collections::HashMap, hash::Hash, net::SocketAddr, num::NonZeroU32, sync::Arc};

use libp2p::{
    core::{ConnectedPoint, Endpoint},
    request_response::{RequestId, ResponseChannel},
};
use tracing::warn;

use crate::{spacetime::SpaceTimeMessage, ManagerEvent, ManagerRef};

use super::PeerId;

/// represents an event coming from the network manager.
/// This is useful for updating your UI when stuff changes on the backend.
/// You can also interact with some events to cause an event.
#[derive(Debug, Clone)]
pub enum Event<TMetadata>
where
    TMetadata: Metadata,
{
    /// add a network interface on this node to listen for
    AddListenAddr(SocketAddr),
    /// remove a network interface from this node so that we don't listen to it
    RemoveListenAddr(SocketAddr),
    /// discovered peer on your local network
    PeerDiscovered(DiscoveredPeer<TMetadata>),
    /// a discovered peer has disappeared from the network
    PeerExpired {
        id: PeerId,
        // Will be none if we receive the expire event without having ever seen a discover event.
        metadata: Option<TMetadata>,
    },
    /// communication was established with a peer.
    /// Theere could actually be multiple connections under the hood but we smooth it over in this API.
    PeerConnected(ConnectedPeer),
    /// communication was lost with a peer.
    PeerDisconnected(PeerId),
}

/// represents a discovered peer. It can be used to get information about the peer or to initiate an action with it.
#[derive(Debug, Clone)]
pub struct DiscoveredPeer<TMetadata>
where
    TMetadata: Metadata,
{
    pub(crate) id: PeerId,
    pub(crate) metadata: TMetadata,
    pub(crate) addresses: Vec<SocketAddr>,
}

impl<TMetadata> DiscoveredPeer<TMetadata>
where
    TMetadata: Metadata,
{
    /// get the peer id of the discovered peer
    pub fn peer_id(&self) -> PeerId {
        self.id
    }

    /// get the metadata of the discovered peer
    pub fn metadata(&self) -> &TMetadata {
        &self.metadata
    }

    /// get the addresses of the discovered peer
    pub fn addresses(&self) -> &Vec<SocketAddr> {
        &self.addresses
    }

    /// dial will queue an event to start a connection with the peer
    pub async fn dial(self, manager: &Arc<ManagerRef<TMetadata>>) {
        match manager
            .internal_tx
            .send(ManagerEvent::Dial(self.id, self.addresses))
            .await
        {
            Ok(_) => {}
            Err(err) => warn!(
                "error queueing up a dial event to peer '{}': {}",
                self.id, err
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConnectionType {
    /// The peer that created the Quic connection. The server in a conventional sense.
    Dialer,
    /// The peer that received the Quic connection. The client in a conventional sense.
    Listener,
}

// TODO: This is coming out wrong. Maybe Quic proto has a bug???
impl From<ConnectedPoint> for ConnectionType {
    fn from(value: ConnectedPoint) -> Self {
        match value {
            ConnectedPoint::Dialer { role_override, .. } => match role_override {
                Endpoint::Dialer => ConnectionType::Dialer,
                Endpoint::Listener => ConnectionType::Listener,
            },
            ConnectedPoint::Listener { .. } => ConnectionType::Listener,
        }
    }
}

/// TODO
#[derive(Debug, Clone)]
pub struct ConnectedPeer {
    pub(crate) active_connections: NonZeroU32,
    pub(crate) conn_type: ConnectionType,
}

impl ConnectedPeer {
    pub fn disconnect<TMetadata: Metadata>(self, manager: &Arc<ManagerRef<TMetadata>>) {
        todo!();
    }
}

/// TODO
pub struct Connection<TMetadata>
where
    TMetadata: Metadata,
{
    pub(crate) manager: Arc<ManagerRef<TMetadata>>,
}

impl<TMetadata> Connection<TMetadata>
where
    TMetadata: Metadata,
{
    pub fn manager(&self) -> &Arc<ManagerRef<TMetadata>> {
        &self.manager
    }
}

/// this trait must be implemented for the metadata type to allow it to be converted to MDNS DNS records.
pub trait Metadata: Clone + Send + Sync + 'static {
    fn to_hashmap(self) -> HashMap<String, String>;

    fn from_hashmap(data: &HashMap<String, String>) -> Result<Self, String>
    where
        Self: Sized;
}
