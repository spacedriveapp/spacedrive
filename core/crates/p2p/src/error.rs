use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	// Network setup errors
	#[error("Setup iroh endpoint: {0}")]
	SetupEndpoint(anyhow::Error),
	#[error("Setup iroh listener: {0}")]
	SetupListener(io::Error),
	#[error("Initialize LocalSwarmDiscovery: {0}")]
	LocalSwarmDiscoveryInit(anyhow::Error),
	#[error("Initialize DhtDiscovery: {0}")]
	DhtDiscoveryInit(anyhow::Error),

	// Known hosts loading errors
	#[error("Serialize known devices: {0}")]
	SerializeKnownDevices(postcard::Error),
	#[error("Deserialize known devices: {0}")]
	DeserializeKnownDevices(postcard::Error),
	#[error("Load known devices from file: {0}")]
	LoadKnownDevices(io::Error),
	#[error("Save known devices to file: {0}")]
	SaveKnownDevices(io::Error),
}
