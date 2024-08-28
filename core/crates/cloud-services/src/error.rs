use sd_cloud_schema::cloud_p2p::Service;
use sd_utils::error::FileIOError;

use std::{io, net::AddrParseError};

use quic_rpc::transport::quinn::QuinnConnection;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("Couldn't parse Cloud Services API address URL: {0}")]
	InvalidUrl(reqwest_middleware::reqwest::Error),
	#[error("Failed to initialize http client: {0}")]
	HttpClientInit(reqwest_middleware::reqwest::Error),
	#[error("Failed to request Cloud Services API address from Auth Server route: {0}")]
	FailedToRequestApiAddress(reqwest_middleware::Error),
	#[error("Auth Server's Cloud Services API address route returned an error: {0}")]
	AuthServerError(reqwest_middleware::reqwest::Error),
	#[error(
		"Failed to extract response body from Auth Server's Cloud Services API address route: {0}"
	)]
	FailedToExtractApiAddress(reqwest_middleware::reqwest::Error),
	#[error("Failed to parse auth server's Cloud Services API address: {0}")]
	FailedToParseApiAddress(#[from] AddrParseError),
	#[error("Failed to create endpoint: {0}")]
	FailedToCreateEndpoint(io::Error),

	// Token refresher errors
	#[error("Failed to decode access token data: {0}")]
	DecodeAccessTokenData(#[from] base64::DecodeError),
	#[error("Failed to deserialize access token json data: {0}")]
	DeserializeAccessTokenData(#[from] serde_json::Error),
	#[error("Token expired")]
	TokenExpired,
	#[error("Failed to request refresh token: {0}")]
	RefreshTokenRequest(reqwest_middleware::Error),
	#[error("Missing tokens on refresh response")]
	MissingTokensOnRefreshResponse,
	#[error("Failed to parse token header value to string: {0}")]
	FailedToParseTokenHeaderValueToString(#[from] reqwest_middleware::reqwest::header::ToStrError),

	// Key Manager errors
	#[error("Failed to handle File on KeyManager: {0}")]
	FileIO(#[from] FileIOError),
	#[error("Failed to handle key store serialization: {0}")]
	KeyStoreSerialization(#[from] postcard::Error),
	#[error("Key store encryption related error: {{context: \"{context}\", source: {source}}}")]
	KeyStoreCrypto {
		#[source]
		source: sd_crypto::Error,
		context: &'static str,
	},
	#[error("Key manager not initialized")]
	KeyManagerNotInitialized,

	// Cloud P2P errors
	#[error("Failed to create Cloud P2P endpoint: {0}")]
	CreateCloudP2PEndpoint(anyhow::Error),
	#[error("Failed to connect to Cloud P2P node: {0}")]
	ConnectToCloudP2PNode(anyhow::Error),
	#[error("Communication error with Cloud P2P node: {0}")]
	CloudP2PRpcCommunication(#[from] quic_rpc::pattern::rpc::Error<QuinnConnection<Service>>),
	#[error("Cloud P2P not initialized")]
	CloudP2PNotInitialized,
}

#[derive(thiserror::Error, Debug)]
pub enum GetTokenError {
	#[error("Token refresher not initialized")]
	RefresherNotInitialized,
	#[error("Token refresher failed to refresh and need to be initialized again")]
	FailedToRefresh,
}

impl From<Error> for rspc::Error {
	fn from(e: Error) -> Self {
		Self::with_cause(rspc::ErrorCode::InternalServerError, e.to_string(), e)
	}
}
