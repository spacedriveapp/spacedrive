use std::{
	collections::HashMap,
	marker::PhantomData,
	sync::{Arc, PoisonError},
};

use thiserror::Error;

use crate::{
	spacetime::UnicastStream, spacetunnel::RemoteIdentity, DiscoveredPeer, Manager, Metadata,
	PeerId,
};

use super::DiscoveryManager;

/// A Service represents a thing your application exposes to the network that can be discovered and connected to.
pub struct Service<TMeta> {
	name: String,
	manager: Arc<DiscoveryManager>,
	phantom: PhantomData<fn() -> TMeta>,
}

impl<TMeta: Metadata> Service<TMeta> {
	pub fn new(
		name: impl Into<String>,
		manager: Arc<DiscoveryManager>,
	) -> Result<Self, ErrDuplicateServiceName> {
		let name = name.into();
		{
			let mut state = manager
				.state
				.write()
				.unwrap_or_else(PoisonError::into_inner);
			if state.services.contains_key(&name) {
				return Err(ErrDuplicateServiceName);
			}
			state.services.insert(name.clone(), Default::default());
		}

		Ok(Self {
			name,
			manager,
			phantom: PhantomData,
		})
	}

	pub fn update(&mut self, meta: TMeta) {
		self.manager
			.state
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.services
			.insert(self.name.clone(), meta.to_hashmap());

		self.manager.rebroadcast();
	}

	pub fn get_state(&self) -> HashMap<RemoteIdentity, PeerStatus> {
		// TODO: Connected peers won't show up

		// let a = self
		// 	.manager
		// 	.state
		// 	.write()
		// 	.unwrap_or_else(PoisonError::into_inner)
		// 	.discovered
		// 	.entry(self.name.clone())
		// 	.or_insert(Default::default())
		// 	.into_iter()
		// 	.map(|(i, p)| (i.clone(), p.clone().into()))
		// 	.collect::<Vec<_>>();

		// let b = self.manager

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

#[derive(Debug, Error)]
#[error("a service has already been mounted with this name")]
pub struct ErrDuplicateServiceName;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum PeerStatus {
	Unavailable,
	Discovered(PeerId),
	Connected(PeerId),
}

impl From<super::RemotePeer> for PeerStatus {
	fn from(value: super::RemotePeer) -> Self {
		match value {
			super::RemotePeer::Unavailable => Self::Unavailable,
			super::RemotePeer::Discovered(c) => Self::Discovered(c.peer_id),
			super::RemotePeer::Connected(c) => Self::Connected(c.peer_id),
		}
	}
}
