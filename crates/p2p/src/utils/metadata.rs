use std::{collections::HashMap, fmt::Debug};

/// this trait must be implemented for the metadata type to allow it to be converted to MDNS DNS records.
#[deprecated]
pub trait Metadata: Debug + Clone + Send + Sync + 'static {
	fn to_hashmap(self) -> HashMap<String, String>;

	fn from_hashmap(data: &HashMap<String, String>) -> Result<Self, String>
	where
		Self: Sized;
}
