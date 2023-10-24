use std::{
	collections::HashMap,
	fmt,
	sync::{Arc, PoisonError, RwLock},
};

use sd_p2p::{spacetunnel::RemoteIdentity, PeerId, PeerStatus, Service};
use streamunordered::StreamUnordered;
use uuid::Uuid;

use super::PeerMetadata;

#[derive(Default)]
pub struct LibraryServices {
	services: RwLock<HashMap<Uuid, Arc<Service<PeerMetadata>>>>,
	// TODO: Hook this up -> Maybe on it's own task
	// events: StreamUnordered<ServiceEvent>,
}

impl fmt::Debug for LibraryServices {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("LibraryServices")
			.field("services", &self.services.read().unwrap().keys())
			.finish()
	}
}

impl LibraryServices {
	pub fn get(&self, id: &Uuid) -> Option<Arc<Service<PeerMetadata>>> {
		self.services
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.get(id)
			.cloned()
	}

	pub fn add(&self) {
		todo!();
	}

	pub fn remove(&self) {
		todo!();
	}

	pub fn libraries(&self) -> Vec<(Uuid, Arc<Service<PeerMetadata>>)> {
		self.services
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.iter()
			.map(|(k, v)| (k.clone(), v.clone()))
			.collect::<Vec<_>>()
	}

	// TODO: `sd_p2p` should be able to handle this internally for us
	pub(super) fn peer_connected(&self, peer_id: PeerId) {
		// TODO: This is a very suboptimal way of doing this cause it assumes a discovery message will always come before discover which is false.
		// TODO: Hence part of the need for `Self::peer_connected2`
		for lib in self
			.services
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.values_mut()
		{
			for instance in lib._get_mut().values_mut() {
				if let PeerStatus::Discovered(id) = instance {
					if *id == peer_id {
						*instance = PeerStatus::Connected(peer_id);
						return; // Will only exist once so we short circuit
					}
				}
			}
		}
	}

	// // TODO: Can this be merged with `peer_connected`???
	pub(super) fn peer_connected2(
		libraries: &LibraryServices,
		instance_id: RemoteIdentity,
		peer_id: PeerId,
	) {
		for lib in libraries
			.services
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.values_mut()
		{
			if let Some(instance) = lib._get_mut().get_mut(&instance_id) {
				*instance = PeerStatus::Connected(peer_id);
				return; // Will only exist once so we short circuit
			}
		}
	}

	// TODO: `sd_p2p` should be able to handle this internally for us
	pub(super) fn peer_disconnected(&self, peer_id: PeerId) {
		for lib in self
			.services
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.values_mut()
		{
			for instance in lib._get_mut().values_mut() {
				if let PeerStatus::Connected(id) = instance {
					if *id == peer_id {
						*instance = PeerStatus::Unavailable;
						return; // Will only exist once so we short circuit
					}
				}
			}
		}
	}

	// TODO: `sd_p2p` should be able to handle this internally for us
	pub(super) fn peer_expired(&self, id: PeerId) {
		for lib in self
			.services
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.values_mut()
		{
			for instance in lib._get_mut().values_mut() {
				if let PeerStatus::Discovered(peer_id) = instance {
					if *peer_id == id {
						*instance = PeerStatus::Unavailable;
					}
				}
			}
		}
	}
}
