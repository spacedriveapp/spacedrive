use std::{collections::HashMap, fmt::Debug};

use crate::PeerId;

/// this trait must be implemented for the metadata type to allow it to be converted to MDNS DNS records.
pub trait Metadata: Debug + Clone + Send + Sync + 'static {
	fn to_hashmap(self) -> HashMap<String, String>;

	fn from_hashmap(peer_id: &PeerId, data: &HashMap<String, String>) -> Result<Self, String>
	where
		Self: Sized;
}
