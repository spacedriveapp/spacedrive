use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
};

use tokio::sync::{mpsc, RwLock};

use crate::{ConnectedPeer, DiscoveredPeer, ManagerEvent, Metadata, PeerId};

/// TODO
#[derive(Debug)]
pub struct ManagerRef<TMetadata>
where
    TMetadata: Metadata,
{
    pub(crate) service_name: String,
    pub(crate) peer_id: PeerId,
    pub(crate) internal_tx: mpsc::Sender<ManagerEvent>,
    pub(crate) listen_addrs: RwLock<HashSet<SocketAddr>>,
    pub(crate) discovered_peers: RwLock<HashMap<PeerId, DiscoveredPeer<TMetadata>>>,
    pub(crate) connected_peers: RwLock<HashMap<PeerId, ConnectedPeer>>,
}
