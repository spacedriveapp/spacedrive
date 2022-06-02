use std::{io, str::FromStr, sync::Arc};

use dashmap::DashMap;
use libp2p::{
	futures::StreamExt,
	identity::Keypair,
	mdns::{Mdns, MdnsConfig, MdnsEvent},
	swarm::SwarmEvent,
	PeerId, Swarm,
};
use serde::{Deserialize, Serialize};
use tokio::{
	select,
	sync::mpsc::{self, channel},
};
use ts_rs::TS;

use crate::{
	node::NodeState,
	prisma::{node, PrismaClient},
};

/// TODO
enum InternalEvent {
	Connect(PeerId),
}

/// NetworkManager is responsible for managing all P2P communication for a single node.
/// The acts as an abstraction to decouple libp2p from the rest of the application.
pub struct NetworkManager {
	// state is used to store data about the node to disk.
	// state: NodeState,
	// db is the database of the current library.
	db: Arc<PrismaClient>,
	// peer_id stores the libp2p peer id of this node.
	peer_id: PeerId,
	// paired_peers holds a list of peer IDs that we are currently paired with. This data structure is eventual consistency.
	paired_peers: DashMap<PeerId, node::Data>,
	// collected_peers holds a list of peer IDs that we are actively connected with. This data structure is eventual consistency.
	connected_peers: DashMap<PeerId, ()>,
	// discovered_peers holds a list of all peers that can be found on the users network. This data structure is eventual consistency.
	discovered_peers: DashMap<PeerId, ()>,
	// TODO
	channel: mpsc::Sender<InternalEvent>,
}

impl NetworkManager {
	/// new will initialise a NetworkManager based on the state passed in.
	pub async fn new(mut state: NodeState, db: Arc<PrismaClient>) -> io::Result<Arc<Self>> {
		let keypair = match state.keypair {
			Some(keypair) => Keypair::from_protobuf_encoding(&keypair).unwrap(),
			None => {
				let keypair = Keypair::generate_ed25519();
				state.keypair = Some(keypair.to_protobuf_encoding().unwrap());
				state.save();
				keypair
			}
		};
		let peer_id = PeerId::from(keypair.public());

		let behaviour = Mdns::new(MdnsConfig::default()).await?;
		let transport = libp2p::development_transport(keypair).await?;
		let swarm = Swarm::new(transport, behaviour, peer_id);

		let paired_peers = DashMap::new();
		for node in db.node().find_many(vec![]).exec().await.unwrap() {
			match PeerId::from_str(&node.pub_id) {
				Ok(peer_id) => {
					paired_peers.insert(peer_id, node);
				}
				Err(err) => {
					println!("Error passing node public ID: {}", err);
					// TODO: This should be a fault but that would be a breaking change.
				}
			}
		}

		let (tx, rx) = channel(50);
		let this = Arc::new(Self {
			db,
			peer_id,
			paired_peers,
			connected_peers: DashMap::new(),
			discovered_peers: DashMap::new(),
			channel: tx,
		});
		tokio::spawn(this.clone().event_loop(swarm, rx));

		Ok(this)
	}

	/// event_loop is run in a separate thread when creating the [NetworkManager]. It is incharge of handling events from libp2p.
	async fn event_loop(
		self: Arc<Self>,
		mut swarm: Swarm<Mdns>,
		mut receiver: mpsc::Receiver<InternalEvent>,
	) {
		swarm
			.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
			.unwrap();

		loop {
			select! {
				event = swarm.select_next_some() => {
					match event {
						SwarmEvent::Behaviour(MdnsEvent::Discovered(peers)) => {
							for (peer, addr) in peers {
								self.discovered_peers.insert(peer, ());

							}
						}
						SwarmEvent::Behaviour(MdnsEvent::Expired(expired)) => {
							for (peer, addr) in expired {
								self.discovered_peers.remove(&peer);
							}
						}
						SwarmEvent::ConnectionEstablished {
							peer_id,
							endpoint,
							num_established,
							concurrent_dial_errors
						} => {
							println!("Connection established to {}. We now have {} peers.", peer_id, num_established);
							self.connected_peers.insert(peer_id, ());
						}
						SwarmEvent::ConnectionClosed{ peer_id, endpoint, num_established, cause } => {
							self.connected_peers.remove(&peer_id);
						}
						SwarmEvent::IncomingConnection { local_addr, send_back_addr } => {
							// TODO
						}
						_ => {} // TODO: Remove this
					}
				}
				event = receiver.recv() => {
					match event {
						Some(event) => match event {
							InternalEvent::Connect(peer_id) => {
								swarm.dial(peer_id).unwrap();
							}
						},
						None => break
					}
				}
			}
		}
	}

	/// peer_id returns the PeerId of the current node.
	pub fn peer_id(&self) -> PeerId {
		self.peer_id.clone()
	}

	/// Pair will pair a new ID with the current node.
	pub async fn pair(&self, remote_peer_id: PeerId) -> Result<(), String> {
		if !self.discovered_peers.contains_key(&remote_peer_id) {
			return Err("Peer not found".to_string());
		}

		let node = self
			.db
			.node()
			.create(
				node::pub_id::set(remote_peer_id.to_base58()),
				node::name::set("todo".to_string()), // TODO: Work this out from the remote client
				vec![],
			)
			.exec()
			.await
			.unwrap();
		self.paired_peers.insert(remote_peer_id, node);

		self.channel
			.send(InternalEvent::Connect(remote_peer_id))
			.await
			.map_err(|_| "error queuing connect command!")?;

		Ok(())
	}

	/// TODO
	pub async fn unpair(&self, remote_peer_id: PeerId) -> Result<(), String> {
		unimplemented!();
	}

	/// Get status will return the current status of the NetworkManager
	pub async fn get_state(&self) -> NetworkManagerState {
		NetworkManagerState {
			peer_id: self.peer_id.to_base58(),
			// paired_peers: self.paired_peers.clone(),
			connected_peers: self
				.connected_peers
				.iter()
				.map(|e| e.key().to_base58())
				.collect(),
			discovered_peers: self
				.discovered_peers
				.iter()
				.map(|e| e.key().to_base58())
				.collect(),
		}
	}
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export)]
pub struct NetworkManagerState {
	peer_id: String,
	// paired_peers: HashMap<String, node::Data>,
	connected_peers: Vec<String>,
	discovered_peers: Vec<String>,
}
