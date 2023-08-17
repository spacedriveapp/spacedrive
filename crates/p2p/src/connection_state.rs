use std::{collections::HashMap, sync::RwLock};

use crate::{spacetunnel::RemoteIdentity, Metadata, PeerId};

/// TODO
#[derive(Debug, Clone, Copy)]
pub enum State {
	Unavailable,
	Discovered(PeerId),
	Connected(PeerId),
}

/// TODO
#[derive(Debug, Default)]
pub struct ConnectionState<T> {
	// TODO: Wrap this is an `ObservableMap` or something like that for frontend updates?
	// TODO: Services coming from discovery
	services: RwLock<HashMap<String /* Service Name */, HashMap<RemoteIdentity, (State, T)>>>,
}

impl<T> ConnectionState<T> {
	// TODO: Insert unavailable instances -> used for over internet discovery

	// // TODO
	// pub fn for_service<T: Metadata>(service: Service2<T>) -> Vec<_>;

	// // TODO
	// pub fn for_remote(identity: RemoteIdentity) -> Vec<_> {}
}
