use sd_cloud_schema::{cloud_p2p, sync::groups, Service};
use sd_utils::error::FileIOError;

use std::{io, net::AddrParseError};

use quic_rpc::{
	pattern::{bidi_streaming, rpc, server_streaming},
	transport::quinn::QuinnConnection,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
	// Setup errors
	#[error("Couldn't parse Cloud Services API address URL: {0}")]
	InvalidUrl(reqwest::Error),
	#[error("Failed to parse Cloud Services API address URL")]
	FailedToParseRelayUrl,
	#[error("Failed to initialize http client: {0}")]
	HttpClientInit(reqwest::Error),
	#[error("Failed to request Cloud Services API address from Auth Server route: {0}")]
	FailedToRequestApiAddress(reqwest_middleware::Error),
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

	// Token refresher errors
	#[error("Invalid token format, missing claims")]
	MissingClaims,
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
	FailedToParseTokenHeaderValueToString(#[from] reqwest::header::ToStrError),

	// Key Manager errors
	#[error("Failed to handle File on KeyManager: {0}")]
	FileIO(#[from] FileIOError),
	#[error("Failed to handle key store serialization: {0}")]
	KeyStoreSerialization(rmp_serde::encode::Error),
	#[error("Failed to handle key store deserialization: {0}")]
	KeyStoreDeserialization(rmp_serde::decode::Error),
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
	CloudP2PRpcCommunication(#[from] rpc::Error<QuinnConnection<cloud_p2p::Service>>),
	#[error("Cloud P2P not initialized")]
	CloudP2PNotInitialized,
	#[error("Failed to initialize LocalSwarmDiscovery: {0}")]
	LocalSwarmDiscoveryInit(anyhow::Error),
	#[error("Failed to initialize DhtDiscovery: {0}")]
	DhtDiscoveryInit(anyhow::Error),

	// Communication errors
	#[error("Failed to communicate with RPC backend: {0}")]
	RpcCommunication(#[from] rpc::Error<QuinnConnection<Service>>),
	#[error("Failed to communicate with Server Streaming RPC backend: {0}")]
	ServerStreamCommunication(#[from] server_streaming::Error<QuinnConnection<Service>>),
	#[error("Failed to receive next response from Server Streaming RPC backend: {0}")]
	ServerStreamRecv(#[from] server_streaming::ItemError<QuinnConnection<Service>>),
	#[error("Failed to communicate with Bidi Streaming RPC backend: {0}")]
	BidiStreamCommunication(#[from] bidi_streaming::Error<QuinnConnection<Service>>),
	#[error("Failed to receive next response from Bidi Streaming RPC backend: {0}")]
	BidiStreamRecv(#[from] bidi_streaming::ItemError<QuinnConnection<Service>>),
	#[error("Error from backend: {0}")]
	Backend(#[from] sd_cloud_schema::Error),
	#[error("Failed to get access token from refresher: {0}")]
	GetToken(#[from] GetTokenError),
	#[error("Unexpected empty response from backend, context: {0}")]
	EmptyResponse(&'static str),
	#[error("Unexpected response from backend, context: {0}")]
	UnexpectedResponse(&'static str),

	// Sync error
	#[error("Sync error: {0}")]
	Sync(#[from] sd_core_sync::Error),
	#[error("Tried to sync messages with a group without having needed key")]
	MissingSyncGroupKey(groups::PubId),
	#[error("Failed to encrypt sync messages: {0}")]
	Encrypt(sd_crypto::Error),
	#[error("Failed to decrypt sync messages: {0}")]
	Decrypt(sd_crypto::Error),
	#[error("Failed to upload sync messages: {0}")]
	UploadSyncMessages(reqwest_middleware::Error),
	#[error("Failed to download sync messages: {0}")]
	DownloadSyncMessages(reqwest_middleware::Error),
	#[error("Received an error response from uploading sync messages: {0}")]
	ErrorResponseUploadSyncMessages(reqwest::Error),
	#[error("Received an error response from downloading sync messages: {0}")]
	ErrorResponseDownloadSyncMessages(reqwest::Error),
	#[error(
		"Received an error response from downloading sync messages while reading its bytes: {0}"
	)]
	ErrorResponseDownloadReadBytesSyncMessages(reqwest::Error),
	#[error("Critical error while uploading sync messages")]
	CriticalErrorWhileUploadingSyncMessages,
	#[error("Failed to send End update to push sync messages")]
	EndUpdatePushSyncMessages(io::Error),
	#[error("Unexpected end of stream while encrypting sync messages")]
	UnexpectedEndOfStream,
	#[error("Failed to create directory to store timestamp keeper files")]
	FailedToCreateTimestampKeepersDirectory(io::Error),
	#[error("Failed to read last timestamp keeper for pulling sync messages: {0}")]
	FailedToReadLastTimestampKeeper(io::Error),
	#[error("Failed to handle last timestamp keeper serialization: {0}")]
	LastTimestampKeeperSerialization(rmp_serde::encode::Error),
	#[error("Failed to handle last timestamp keeper deserialization: {0}")]
	LastTimestampKeeperDeserialization(rmp_serde::decode::Error),
	#[error("Failed to write last timestamp keeper for pulling sync messages: {0}")]
	FailedToWriteLastTimestampKeeper(io::Error),
	#[error("Sync messages download and decrypt task panicked")]
	SyncMessagesDownloadAndDecryptTaskPanicked,
	#[error("Serialization failure to push sync messages: {0}")]
	SerializationFailureToPushSyncMessages(rmp_serde::encode::Error),
	#[error("Deserialization failure to pull sync messages: {0}")]
	DeserializationFailureToPullSyncMessages(rmp_serde::decode::Error),
	#[error("Read nonce stream decryption: {0}")]
	ReadNonceStreamDecryption(io::Error),
	#[error("Incomplete download bytes sync messages")]
	IncompleteDownloadBytesSyncMessages,

	// Temporary errors
	#[error("Device missing secret key for decrypting sync messages")]
	MissingKeyHash,
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

impl From<GetTokenError> for rspc::Error {
	fn from(e: GetTokenError) -> Self {
		Self::with_cause(rspc::ErrorCode::InternalServerError, e.to_string(), e)
	}
}
