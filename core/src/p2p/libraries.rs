use std::{
	collections::HashMap,
	fmt,
	sync::{Arc, PoisonError, RwLock},
};

use sd_p2p::{spacetunnel::RemoteIdentity, DiscoveredPeer, PeerId, PeerStatus, Service};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::library::Library;

use super::PeerMetadata;

pub struct LibraryServices {
	services: RwLock<HashMap<Uuid, Arc<Service<PeerMetadata>>>>,
	tx: broadcast::Sender<()>,
}

impl fmt::Debug for LibraryServices {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("LibraryServices")
			.field("services", &self.services.read().unwrap().keys())
			.finish()
	}
}

impl LibraryServices {
	pub fn new(tx: broadcast::Sender<()>) -> Self {
		Self {
			services: Default::default(),
			tx,
		}
	}

	pub fn get(&self, id: &Uuid) -> Option<Arc<Service<PeerMetadata>>> {
		self.services
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.get(id)
			.cloned()
	}

	pub fn libraries(&self) -> Vec<(Uuid, Arc<Service<PeerMetadata>>)> {
		self.services
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.iter()
			.map(|(k, v)| (k.clone(), v.clone()))
			.collect::<Vec<_>>()
	}

	pub(super) fn update(&self) {
		todo!();
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
						self.tx.send(()).ok();
						return; // Will only exist once so we short circuit
					}
				}
			}
		}
	}

	// // TODO: Can this be merged with `peer_connected`???
	pub(super) fn peer_connected2(&self, instance_id: RemoteIdentity, peer_id: PeerId) {
		for lib in self
			.services
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.values_mut()
		{
			if let Some(instance) = lib._get_mut().get_mut(&instance_id) {
				*instance = PeerStatus::Connected(peer_id);
				self.tx.send(()).ok();
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
						self.tx.send(()).ok();
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
						self.tx.send(()).ok();
					}
				}
			}
		}
	}

	pub(super) async fn peer_discovered(&self, event: DiscoveredPeer<PeerMetadata>) {
		let mut should_connect = false;
		// for lib in self
		// 	.services
		// 	.write()
		// 	.unwrap_or_else(PoisonError::into_inner)
		// 	.values_mut()
		// {
		// 	if let Some((_pk, instance)) = lib
		// 		._get_mut()
		// 		.iter_mut()
		// 		.find(|(pk, _)| event.metadata.instances.iter().any(|pk2| *pk2 == **pk))
		// 	{
		// 		if !matches!(instance, PeerStatus::Connected(_)) {
		// 			should_connect = matches!(instance, PeerStatus::Unavailable);

		// 			*instance = PeerStatus::Discovered(event.peer_id);
		// 		}

		// 		break; // PK can only exist once so we short circuit
		// 	}
		// }
		todo!();

		// We do this here not in the loop so the future can be `Send`
		if should_connect {
			event.dial().await;
		}
	}

	pub(crate) async fn load_library(&self, library: &Library) {
		// let (db_owned_instances, db_instances): (Vec<_>, Vec<_>) = library
		// 	.db
		// 	.instance()
		// 	.find_many(vec![])
		// 	.exec()
		// 	.await
		// 	.unwrap()
		// 	.into_iter()
		// 	.partition_map(
		// 		// TODO: Error handling
		// 		|i| match IdentityOrRemoteIdentity::from_bytes(&i.identity).unwrap() {
		// 			IdentityOrRemoteIdentity::Identity(identity) => Either::Left(identity),
		// 			IdentityOrRemoteIdentity::RemoteIdentity(identity) => Either::Right(identity),
		// 		},
		// 	);

		// let mut libraries = manager
		// 	.libraries
		// 	.write()
		// 	.unwrap_or_else(PoisonError::into_inner);

		// // `self.owned_instances` exists so this call to `load_library` does override instances of other libraries.
		// if db_owned_instances.len() != 1 {
		// 	panic!(
		// 		"Library has '{}' owned instance! Something has gone very wrong!",
		// 		db_owned_instances.len()
		// 	);
		// }
		// owned_instances.insert(library.id, db_owned_instances[0].to_remote_identity());

		// TODO: Maintain old data.
		// let mut old_data = libraries.remove(&library.id);
		// libraries.insert(
		// 	library.id,
		// 	Service::new(),
		// 	LibraryData {
		// 		// We register all remote instances to track connection state(`IdentityOrRemoteIdentity::RemoteIdentity`'s only).
		// 		instances: db_instances
		// 			.into_iter()
		// 			.map(|identity| {
		// 				(
		// 					identity.clone(),
		// 					match old_data
		// 						.as_mut()
		// 						.and_then(|d| d.instances.remove(&identity))
		// 					{
		// 						Some(data) => data,
		// 						None => InstanceState::Unavailable,
		// 					},
		// 				)
		// 			})
		// 			.collect(),
		// 	},
		// );

		// self.p2p
		// 	.update_metadata(owned_instances.values().cloned().collect::<Vec<_>>())
		// 	.await;
	}

	pub(crate) async fn edit_library(&self, _library: &Library) {
		// TODO: Send changes to all connected nodes!

		// TODO: Update mdns
	}

	pub(crate) async fn delete_library(&self, library: &Library) {
		// // Lock them together to ensure changes to both become visible to readers at the same time
		// let mut libraries = self.libraries.write().await;
		// let mut owned_instances = self.owned_instances.write().await;

		// // TODO: Do proper library delete/unpair procedure.
		// libraries.remove(&library.id);
		// owned_instances.remove(&library.id);
		// self.p2p
		// 	.update_metadata(owned_instances.values().cloned().collect::<Vec<_>>())
		// 	.await;
	}
}
