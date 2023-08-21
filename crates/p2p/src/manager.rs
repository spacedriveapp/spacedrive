use std::{
	collections::HashMap,
	sync::{
		atomic::{AtomicBool, AtomicU64},
		Arc,
	},
};

use libp2p::{
	core::{muxing::StreamMuxerBox, Transport},
	swarm::SwarmBuilder,
};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, warn};

use crate::{
	spacetime::{SpaceTime, UnicastStream},
	Component, ConnectionState, Keypair, ManagerStream, ManagerStreamAction, Metadata, PeerId,
	Service,
};

#[derive(Debug, Clone)]
pub(crate) struct EventStreamTx(mpsc::Sender<ManagerStreamAction>);

impl EventStreamTx {
	pub(crate) async fn emit(&self, event: ManagerStreamAction) {
		match self.0.send(event).await {
			Ok(_) => {}
			Err(err) => warn!("error emitting event: {}", err),
		}
	}
}

/// Is the core component of the P2P system that holds the state and delegates actions to the other components
#[derive(Debug)]
pub struct Manager {
	pub(crate) peer_id: PeerId,
	pub(crate) application_name: &'static str,
	pub(crate) spacetime_name: String,
	pub(crate) stream_id: AtomicU64,

	// TODO: Expose generic
	pub(crate) connection_state: Arc<ConnectionState<()>>,

	event_stream_tx: EventStreamTx,
}

impl Manager {
	/// create a new P2P manager. Please do your best to make the callback closures as fast as possible because they will slow the P2P event loop!
	pub async fn new(
		application_name: &'static str,
		keypair: &Keypair,
	) -> Result<(Arc<Self>, ManagerStream), ManagerError> {
		application_name
			.chars()
			.all(|c| char::is_alphanumeric(c) || c == '-')
			.then_some(())
			.ok_or(ManagerError::InvalidAppName)?;

		let peer_id = PeerId(keypair.raw_peer_id());
		let (event_stream_tx, event_stream_rx) = mpsc::channel(1024);

		let this = Arc::new(Self {
			application_name,
			spacetime_name: format!("/{}/spacetime/1.0.0", application_name),
			stream_id: AtomicU64::new(0),
			peer_id,
			connection_state: Arc::new(Default::default()),
			event_stream_tx: EventStreamTx(event_stream_tx),
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
			this,
			ManagerStream {
				event_stream_rx,
				swarm,
				queued_events: Default::default(),
				shutdown: AtomicBool::new(false),
				on_establish_streams: HashMap::new(),
				services: Default::default(),
			},
		))
	}

	pub(crate) async fn emit(&self, event: ManagerStreamAction) {
		self.event_stream_tx.emit(event).await
	}

	/// TODO: Docs
	// // Construct or load a service.
	// pub fn service<T: Metadata>(&self, name: String, identifier: I, metadata: T) -> Service<I, T> {
	// 	todo!();
	// }

	pub fn connection_state(&self) -> Arc<ConnectionState<()>> {
		self.connection_state.clone()
	}

	// TODO: This being `async` is cringe
	pub async fn component(&self, service: impl Component) {
		self.emit(ManagerStreamAction::RegisterComponent(Box::pin(service)))
			.await;
	}

	pub fn peer_id(&self) -> PeerId {
		self.peer_id
	}

	pub async fn get_connected_peers(&self) -> Result<Vec<PeerId>, ()> {
		let (tx, rx) = oneshot::channel();
		self.emit(ManagerStreamAction::GetConnectedPeers(tx)).await;
		rx.await.map_err(|_| {
			warn!("failed to get connected peers 3 times, returning error");
		})
	}

	// TODO: Does this need any timeouts to be added cause hanging forever is bad?
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
			.emit(ManagerStreamAction::Shutdown(tx))
			.await;
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
