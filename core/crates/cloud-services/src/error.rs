use std::{io, net::AddrParseError};

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("Couldn't parse Cloud Services API address URL: {0}")]
	InvalidUrl(reqwest::Error),
	#[error("Failed to initialize http client: {0}")]
	HttpClientInit(reqwest::Error),
	#[error("Failed to request Cloud Services API address from Auth Server route: {0}")]
	FailedToRequestApiAddress(reqwest::Error),
	#[error("Auth Server's Cloud Services API address route returned an error: {0}")]
	AuthServerError(reqwest::Error),
	#[error(
		"Failed to extract response body from Auth Server's Cloud Services API address route: {0}"
	)]
	FailedToExtractApiAddress(reqwest::Error),
	#[error("Failed to parse auth server's Cloud Services API address: {0}")]
	FailedToParseApiAddress(#[from] AddrParseError),
	#[error("Failed to create endpoint: {0}")]
	FailedToCreateEndpoint(io::Error),
}

impl From<Error> for rspc::Error {
	fn from(e: Error) -> Self {
		rspc::Error::with_cause(rspc::ErrorCode::InternalServerError, e.to_string(), e)
	}
}
