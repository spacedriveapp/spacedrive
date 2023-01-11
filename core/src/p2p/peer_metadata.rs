use std::collections::HashMap;

use sd_p2p::Metadata;

#[derive(Debug, Clone)]
pub struct PeerMetadata {
	pub(super) name: String,
	// TODO: Add OS and Spacedrive app version
}

impl Metadata for PeerMetadata {
	fn to_hashmap(self) -> HashMap<String, String> {
		HashMap::from([("name".to_owned(), self.name)])
	}

	fn from_hashmap(data: &HashMap<String, String>) -> Result<Self, String>
	where
		Self: Sized,
	{
		Ok(Self {
			name: data
				.get("name")
				.ok_or_else(|| {
					"DNS record for field 'name' missing. Unable to decode 'PeerMetadata'!"
						.to_owned()
				})?
				.to_owned(),
		})
	}
}
