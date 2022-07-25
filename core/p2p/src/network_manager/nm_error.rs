use std::io;

use thiserror::Error;

/// Represents an error that occurs while initalising the [crate::NetworkManager].
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
	#[error("error generating P2P identity")]
	RcGen(#[from] rcgen::RcgenError),
}
