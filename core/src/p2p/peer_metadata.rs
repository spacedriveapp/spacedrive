use std::{collections::HashMap, env, str::FromStr};

use rspc::Type;
use sd_p2p::Metadata;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct PeerMetadata {
	pub(super) name: String,
	pub(super) operating_system: Option<OperationSystem>,
	pub(super) version: Option<String>,
}

impl Metadata for PeerMetadata {
	fn to_hashmap(self) -> HashMap<String, String> {
		HashMap::from([
			("name".to_owned(), self.name),
			("os".to_owned(), self.operating_system),
			("version".to_owned(), self.version),
		])
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
			operating_system: data
				.get("os")
				.map(|os| os.parse().map_err(|_| "Unable to parse 'OperationSystem'!"))
				.transpose()?,
			version: data.get("version").map(|v| v.to_owned()),
		})
	}
}

/// Represents the operating system which the remote peer is running.
/// This is not used internally and predominantly is designed to be used for display purposes by the embedding application.
#[derive(Debug, Clone, Type, Serialize, Deserialize)]
pub enum OperationSystem {
	Windows,
	Linux,
	MacOS,
	IOS,
	Android,
	Other(String),
}

impl OperationSystem {
	pub fn get_os() -> Self {
		match env::consts::OS {
			"windows" => OperationSystem::Windows,
			"macos" => OperationSystem::MacOS,
			"linux" => OperationSystem::Linux,
			"ios" => OperationSystem::IOS,
			"android" => OperationSystem::Android,
			platform => OperationSystem::Other(platform.into()),
		}
	}
}

impl From<OperationSystem> for String {
	fn from(os: OperationSystem) -> Self {
		match os {
			OperationSystem::Windows => "Windows".into(),
			OperationSystem::Linux => "Linux".into(),
			OperationSystem::MacOS => "MacOS".into(),
			OperationSystem::IOS => "IOS".into(),
			OperationSystem::Android => "Android".into(),
			OperationSystem::Other(s) => {
				let mut chars = s.chars();
				chars.next();
				chars.as_str().to_string()
			}
		}
	}
}

impl FromStr for OperationSystem {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let mut chars = s.chars();
		match chars.next() {
			Some('w') => Ok(OperationSystem::Windows),
			Some('l') => Ok(OperationSystem::Linux),
			Some('m') => Ok(OperationSystem::MacOS),
			Some('i') => Ok(OperationSystem::IOS),
			Some('a') => Ok(OperationSystem::Android),
			_ => Ok(OperationSystem::Other(chars.as_str().to_string())),
		}
	}
}
