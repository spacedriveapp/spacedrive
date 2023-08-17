use std::{
	collections::{HashMap, HashSet},
	net::SocketAddr,
	pin::Pin,
	sync::{Arc, PoisonError, RwLock},
	task::{Context, Poll},
};

use crate::{
	Component, DiscoveredPeer, InternalEvent, Manager, Mdns, Metadata, MetadataManager, PeerId,
};

/// TODO
pub struct Discovery<TMetadata: Metadata> {
	listen_addrs: RwLock<HashSet<SocketAddr>>,
	discovered: RwLock<HashMap<PeerId, DiscoveredPeer<TMetadata>>>,
	metadata_manager: Arc<MetadataManager<TMetadata>>,
	mdns: Mdns<TMetadata>,
}

impl<TMetadata: Metadata> Discovery<TMetadata> {
	pub fn new(
		manager: &Manager<TMetadata>,
		metadata_manager: Arc<MetadataManager<TMetadata>>,
	) -> Self {
		Self {
			listen_addrs: Default::default(),
			discovered: Default::default(),
			metadata_manager: metadata_manager.clone(),
			mdns: Mdns::new(manager.application_name, manager.peer_id, metadata_manager)
				// TODO: Error handling
				.unwrap(),
		}
	}

	pub fn listen_addrs(&self) -> HashSet<SocketAddr> {
		self.listen_addrs
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.clone()
	}

	pub async fn get_discovered_peers(&self) -> Vec<DiscoveredPeer<TMetadata>> {
		self.discovered
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.values()
			.cloned()
			.collect()
	}
}

impl<TMetadata: Metadata> Component for Discovery<TMetadata> {
	fn advertise(self: Pin<&mut Self>) {
		// self.mdns.queue_advertisement();
		todo!();
	}

	fn get_candidates(self: Pin<&mut Self>, peer_id: PeerId, candidates: &mut Vec<SocketAddr>) {
		candidates.extend(
			self.discovered
				.read()
				.unwrap_or_else(PoisonError::into_inner)
				.get(&peer_id)
				.unwrap()
				.addresses
				.clone(),
		);
	}

	fn on_event(self: Pin<&mut Self>, event: InternalEvent) {
		match event {
			InternalEvent::NewListenAddr(addr) => {
				self.listen_addrs
					.write()
					.unwrap_or_else(PoisonError::into_inner)
					.insert(addr);
			}
			InternalEvent::ExpiredListenAddr(addr) => {
				self.listen_addrs
					.write()
					.unwrap_or_else(PoisonError::into_inner)
					.remove(&addr);
			}
			InternalEvent::Shutdown => {
				self.mdns.shutdown();
			}
			// TODO: non_exhaustive is broken, da hell
			_ => {}
		}
	}

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
		// let y = Box::pin(self.mdns.poll(cx));

		// let y = self.mdns.poll(manager);

		// TODO: Poll `self.mdns` properly

		Poll::Pending
	}
}
