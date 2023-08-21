use std::collections::HashMap;

use uuid::Uuid;

pub trait ServiceIdentifier {}

impl ServiceIdentifier for String {}

impl ServiceIdentifier for Uuid {}

/// this trait must be implemented for the metadata type to allow it to be converted to MDNS DNS records.
pub trait Metadata: Clone + Send + Sync + 'static {
	fn to_hashmap(self) -> HashMap<String, String>;

	fn from_hashmap(data: &HashMap<String, String>) -> Result<Self, String>
	where
		Self: Sized;
}

#[derive(Debug)]
pub struct Service<I, T>
where
	I: ServiceIdentifier,
	T: Metadata,
{
	/// Name of the service being provided
	service_name: &'static str,
	/// Unique identifier for this current instance of the service
	identifier: I,
	/// Metadata for this service
	metadata: T,
	/// Peers that have been discovered to provide this service
	discovered: HashMap<I, T>,
	// TODO: Known peers for over internet connections?
}

impl<I, T> Service<I, T>
where
	I: ServiceIdentifier,
	T: Metadata,
{
	// pub fn new(manager: &()) -> Self {}

	// pub fn get();

	// pub fn update() {
	// 	self.discovery_tx.send(()).unwrap();
	// }

	// TODO: Subscribe to events for this service and serve them via rspc

	// TODO: Accessors for discovered peers and their metadata
}
