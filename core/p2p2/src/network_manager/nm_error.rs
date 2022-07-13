use std::io;

use thiserror::Error;

/// NetworkManagerError represents an error that occurred while creating a new NetworkManager.
#[derive(Error, Debug)]
pub enum NetworkManagerError {
	// TODO: Cleanup the names of the errors
	#[error("the application name your provided is invalid. Ensure it is alphanumeric!")]
	InvalidAppName,
	#[error("error starting the mDNS service")]
	MDNSDaemon(#[from] mdns_sd::Error),
	#[error("error attaching the shutdown handler")]
	ShutdownHandler(#[from] ctrlc::Error),
	#[error("error starting the if_watch service")]
	IfWatch(io::Error),
	#[error("error configuring certificates for the P2P server")]
	Crypto(#[from] rustls::Error),
	#[error("error starting P2P server")]
	Server(io::Error),
}
