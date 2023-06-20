use std::{
	collections::{HashMap, HashSet},
	net::SocketAddr,
	sync::{
		atomic::{AtomicBool, AtomicU64},
		Arc,
	},
};

use libp2p::{core::muxing::StreamMuxerBox, swarm::SwarmBuilder, Transport};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, warn};

use crate::{
	spacetime::{SpaceTime, UnicastStream},
	DiscoveredPeer, Keypair, ManagerStream, ManagerStreamAction, Mdns, MdnsState, Metadata,
	MetadataManager, PeerId,
};

/// Is the core component of the P2P system that holds the state and delegates actions to the other components
#[derive(Debug)]
pub struct Manager<TMetadata: Metadata> {
	pub(crate) mdns_state: Arc<MdnsState<TMetadata>>,
	pub(crate) peer_id: PeerId,
	pub(crate) application_name: &'static [u8],
	pub(crate) stream_id: AtomicU64,
	event_stream_tx: mpsc::Sender<ManagerStreamAction<TMetadata>>,
}

impl<TMetadata: Metadata> Manager<TMetadata> {
	/// create a new P2P manager. Please do your best to make the callback closures as fast as possible because they will slow the P2P event loop!
	pub async fn new(
		application_name: &'static str,
		keypair: &Keypair,
		metadata_manager: Arc<MetadataManager<TMetadata>>,
	) -> Result<(Arc<Self>, ManagerStream<TMetadata>), ManagerError> {
		application_name
			.chars()
			.all(|c| char::is_alphanumeric(c) || c == '-')
			.then_some(())
			.ok_or(ManagerError::InvalidAppName)?;

		let peer_id = PeerId(keypair.raw_peer_id());
		let (event_stream_tx, event_stream_rx) = mpsc::channel(1024);

		let (mdns, mdns_state) = Mdns::new(application_name, peer_id, metadata_manager)
			.await
			.unwrap();
		let this = Arc::new(Self {
			mdns_state,
			// Look this is bad but it's hard to avoid. Technically a memory leak but it's a small amount of memory and is should done on startup on the P2P system.
			application_name: Box::leak(Box::new(
				format!("/{}/spacetime/1.0.0", application_name)
					.as_bytes()
					.to_vec(),
			)),
			stream_id: AtomicU64::new(0),
			peer_id,
			event_stream_tx,
		});

		let mut swarm = SwarmBuilder::with_tokio_executor(
			libp2p_quic::GenTransport::<libp2p_quic::tokio::Provider>::new(
				libp2p_quic::Config::new(&keypair.inner()),
			)
			.map(|(p, c), _| (p, StreamMuxerBox::new(c)))
			.boxed(),
			SpaceTime::new(this.clone()),
			keypair.raw_peer_id(),
		)
		.build();
		{
			let listener_id = swarm
            .listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse().expect("Error passing libp2p multiaddr. This value is hardcoded so this should be impossible."))
            .unwrap();
			debug!("created ipv4 listener with id '{:?}'", listener_id);
		}
		{
			let listener_id = swarm
        .listen_on("/ip6/::/udp/0/quic-v1".parse().expect("Error passing libp2p multiaddr. This value is hardcoded so this should be impossible."))
        .unwrap();
			debug!("created ipv4 listener with id '{:?}'", listener_id);
		}

		Ok((
			this.clone(),
			ManagerStream {
				manager: this,
				event_stream_rx,
				swarm,
				mdns,
				queued_events: Default::default(),
				shutdown: AtomicBool::new(false),
				on_establish_streams: HashMap::new(),
			},
		))
	}

	pub(crate) async fn emit(&self, event: ManagerStreamAction<TMetadata>) {
		match self.event_stream_tx.send(event).await {
			Ok(_) => {}
			Err(err) => warn!("error emitting event: {}", err),
		}
	}

	pub fn peer_id(&self) -> PeerId {
		self.peer_id
	}

	pub async fn listen_addrs(&self) -> HashSet<SocketAddr> {
		self.mdns_state.listen_addrs.read().await.clone()
	}

	pub async fn get_discovered_peers(&self) -> Vec<DiscoveredPeer<TMetadata>> {
		self.mdns_state
			.discovered
			.read()
			.await
			.values()
			.cloned()
			.collect()
	}

	pub async fn get_connected_peers(&self) -> Result<Vec<PeerId>, ()> {
		let (tx, rx) = oneshot::channel();
		self.emit(ManagerStreamAction::GetConnectedPeers(tx)).await;
		rx.await.map_err(|_| {
			warn!("failed to get connected peers 3 times, returning error");
		})
	}

	#[allow(clippy::unused_unit)] // TODO: Remove this clippy override once error handling is added
	pub async fn stream(&self, peer_id: PeerId) -> Result<UnicastStream, ()> {
		// TODO: With this system you can send to any random peer id. Can I reduce that by requiring `.connect(peer_id).unwrap().send(data)` or something like that.
		let (tx, rx) = oneshot::channel();
		self.emit(ManagerStreamAction::StartStream(peer_id, tx))
			.await;
		let mut stream = rx.await.map_err(|_| {
			warn!("failed to queue establishing stream to peer '{peer_id}'!");

			()
		})?;
		stream.write_discriminator().await.unwrap(); // TODO: Error handling
		Ok(stream)
	}

	pub async fn broadcast(&self, data: Vec<u8>) {
		self.emit(ManagerStreamAction::BroadcastData(data)).await;
	}

	pub async fn shutdown(&self) {
		let (tx, rx) = oneshot::channel();
		self.event_stream_tx
			.send(ManagerStreamAction::Shutdown(tx))
			.await
			.unwrap();
		rx.await.unwrap_or_else(|_| {
			warn!("Error receiving shutdown signal to P2P Manager!");
		}); // Await shutdown so we don't kill the app before the Mdns broadcast
	}
}

#[derive(Error, Debug)]
pub enum ManagerError {
	#[error(
		"the application name you application provided is invalid. Ensure it is alphanumeric!"
	)]
	InvalidAppName,
	#[error("error with mdns discovery: {0}")]
	Mdns(#[from] mdns_sd::Error),
}
