use std::io;

use thiserror::Error;

/// NetworkManagerError represents an error that occurred while creating a new NetworkManager.
#[derive(Error, Debug)]
pub enum NetworkManagerError {
	#[error("the application name your provided is invalid. Ensure it is alphanumeric!")]
	InvalidAppName,
	#[error("error starting the mDNS service")]
	MDNSDaemon(mdns_sd::Error),
	#[error("error attaching the shutdown handler")]
	ShutdownHandler(ctrlc::Error),
	#[error("error starting the if_watch service")]
	IfWatch(io::Error),
	#[error("error setting up certificates for the QUIC server")]
	Crypto(rustls::Error),
	#[error("error starting QUIC server")]
	Server(io::Error),
}
