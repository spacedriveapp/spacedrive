use std::{
	collections::HashMap,
	sync::{mpsc, Arc},
};

use crate::{
	spacetime::UnicastStream,
	spacetunnel::{Identity, RemoteIdentity},
	DiscoveredPeer, Manager, Metadata, PeerId,
};

use super::DiscoveryManager;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum PeerStatus {
	Unavailable,
	Discovered(PeerId),
	Connected(PeerId),
}

// TODO: Allow pushing expected devices into the Service. Like we will need for the relay to work.

/// A Service represents a // TODO
pub struct Service<TMeta> {
	meta: Option<TMeta>,
	manager: Arc<DiscoveryManager>,
}

// TODO: Service per library or per application?

impl<TMeta: Metadata> Service<TMeta> {
	pub fn new(name: impl Into<String>, identity: Identity) -> Result<Self, ()> {
		let name = name.into();

		// TODO: Deal with duplicate `name`

		// Ok(Self { meta: None })
		todo!();
	}

	// TODO: Hook this up to rest of the app
	pub fn update(&mut self, meta: TMeta) {
		self.meta = Some(meta);

		// self.manager.

		todo!(); // TODO: Tell manager to rebroadcast
	}

	pub fn get_state(&self) -> HashMap<RemoteIdentity, PeerStatus> {
		todo!();
	}

	// TODO: Remove in favor of `get_state` maybe???
	pub fn get_discovered(&self) -> Vec<DiscoveredPeer<TMeta>> {
		// TODO: Get updates from manager

		// TODO: Maybe helper for connecting to incoming peer???

		todo!();
	}

	pub async fn connect(
		&self,
		manager: Arc<Manager<TMeta>>,
		identity: &RemoteIdentity,
	) -> Result<UnicastStream, ()> {
		// TODO: Reject connecting to self or a peer not on this service

		let peer_id = todo!();

		// TODO: Error handling
		let stream = manager.stream(peer_id).await.unwrap(); // TODO: handle providing incorrect peer id
		Ok(stream)
	}

	pub fn subscribe(&self, handler: impl Fn(DiscoveredPeer<TMeta>)) {
		let handler: Box<dyn Fn(_)> = Box::new(handler);

		todo!();
	}

	// pub fn connect(&self) {}
}

// TODO: All theses methods are for incremental migration of `NetworkedLibraries`. They should be removed!
impl<TMeta: Metadata> Service<TMeta> {
	// TODO: Mutex lock on the data???
	pub fn _get(&self) -> &HashMap<RemoteIdentity, PeerStatus> {
		todo!();
	}

	// TODO: Mutex lock on the data???
	pub fn _get_mut(&self) -> &mut HashMap<RemoteIdentity, PeerStatus> {
		todo!();
	}
}

impl<Meta> Drop for Service<Meta> {
	fn drop(&mut self) {
		// TODO: Remove from manager
	}
}
