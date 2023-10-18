use std::{
	collections::HashMap,
	sync::{Arc, RwLock},
};

use crate::spacetunnel::RemoteIdentity;

type ServiceName = String;

/// TODO
pub struct DiscoveryManager {
	/// A list of services the current node is advertising
	services: RwLock<HashMap<ServiceName, HashMap<String, String>>>,
	/// A map of discovered devices and their metadata for each registered service
	discovered: HashMap<ServiceName, HashMap<RemoteIdentity, HashMap<String, String>>>,
	/// A map of known peers which should be connected to if found
	/// This is designed around the Relay/NAT hole punching service where we need to emit who we wanna discover
	known: HashMap<ServiceName, Vec<RemoteIdentity>>,
}

impl DiscoveryManager {
	pub(crate) fn new() -> Arc<Self> {
		Arc::new(Self {
			services: todo!(),
			discovered: todo!(),
			known: todo!(),
		})
	}

	// pub fn register_service() {}
}
