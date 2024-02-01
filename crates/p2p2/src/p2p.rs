use std::{
	collections::HashMap,
	sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use crate::{Identity, RemoteIdentity};

/// Manager for the entire P2P system.
#[derive(Debug)]
pub struct P2P {
	/// A unique identifier for the application.
	/// This will differentiate between different applications using this same P2P library.
	app_name: &'static str,
	/// The identity of the local node.
	/// This is the public/private keypair used to uniquely identify the node.
	identity: Identity,
	/// A metadata being shared from the local node to the remote nodes.
	/// This will contain information such as the node's name, version, and services we provide.
	service: Arc<RwLock<HashMap<String, String>>>,
	/// A list of all discovered nodes and their metadata (which comes from `self.service` above).
	discovered: Arc<RwLock<HashMap<RemoteIdentity, HashMap<String, String>>>>,
}

impl P2P {
	pub fn new(app_name: &'static str, identity: Identity) -> Self {
		P2P {
			app_name,
			identity,
			service: Default::default(),
			discovered: Default::default(),
		}
	}

	pub fn app_name(&self) -> &'static str {
		self.app_name
	}

	pub fn identity(&self) -> &Identity {
		&self.identity
	}

	pub fn remote_identity(&self) -> RemoteIdentity {
		self.identity.to_remote_identity()
	}

	pub fn service(&self) -> RwLockReadGuard<HashMap<String, String>> {
		self.service.read().unwrap_or_else(PoisonError::into_inner)
	}

	pub fn service_mut(&self) -> RwLockWriteGuard<HashMap<String, String>> {
		self.service.write().unwrap_or_else(PoisonError::into_inner)
	}

	pub fn discovered(&self) -> RwLockReadGuard<HashMap<RemoteIdentity, HashMap<String, String>>> {
		self.discovered
			.read()
			.unwrap_or_else(PoisonError::into_inner)
	}

	// TODO: Subscribe to discovered & allow triggering a connection

	pub fn shutdown(&self) {
		// TODO: Properly trigger mDNS shutdown
	}
}
