use crate::p2p::{NotifyUser, UserResponse};

use sd_cloud_schema::{Client, Request, Response, ServicesALPN};

use std::{net::SocketAddr, sync::Arc, time::Duration};

use futures::Stream;
use iroh_net::relay::RelayUrl;
use quic_rpc::{transport::quinn::QuinnConnection, RpcClient, RpcMessage};
use quinn::{crypto::rustls::QuicClientConfig, ClientConfig, Endpoint};
use reqwest::{IntoUrl, Url};
use reqwest_middleware::{reqwest, ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use tokio::sync::{Mutex, RwLock};
use tracing::warn;

use super::{
	error::Error, key_manager::KeyManager, p2p::CloudP2P, token_refresher::TokenRefresher,
};

#[derive(Debug, Default, Clone)]
enum ClientState<In: RpcMessage, Out: RpcMessage> {
	#[default]
	NotConnected,
	Connected(Client<QuinnConnection<In, Out>>),
}

/// Cloud services are a optional feature that allows you to interact with the cloud services
/// of Spacedrive.
/// They're optional in two different ways:
/// - The cloud services depends on a user being logged in with our server.
/// - The user being connected to the internet to begin with.
///
/// As we don't want to force the user to be connected to the internet, we have to make sure
/// that core can always operate without the cloud services.
#[derive(Debug)]
pub struct CloudServices {
	client_state: Arc<RwLock<ClientState<Response, Request>>>,
	get_cloud_api_address: Url,
	http_client: ClientWithMiddleware,
	domain_name: String,
	pub cloud_p2p_dns_origin_name: String,
	pub cloud_p2p_relay_url: RelayUrl,
	pub cloud_p2p_dns_pkarr_url: Url,
	pub token_refresher: TokenRefresher,
	key_manager: Arc<RwLock<Option<Arc<KeyManager>>>>,
	cloud_p2p: Arc<RwLock<Option<Arc<CloudP2P>>>>,
	pub(crate) notify_user_tx: flume::Sender<NotifyUser>,
	notify_user_rx: flume::Receiver<NotifyUser>,
	user_response_tx: flume::Sender<UserResponse>,
	pub(crate) user_response_rx: flume::Receiver<UserResponse>,
	pub has_bootstrapped: Arc<Mutex<bool>>,
}

impl CloudServices {
	/// Creates a new cloud services client that can be used to interact with the cloud services.
	/// The client will try to connect to the cloud services on a best effort basis, as the user
	/// might not be connected to the internet.
	/// If the client fails to connect, it will try again the next time it's used.
	pub async fn new(
		get_cloud_api_address: impl IntoUrl + Send,
		cloud_p2p_relay_url: impl IntoUrl + Send,
		cloud_p2p_dns_pkarr_url: impl IntoUrl + Send,
		cloud_p2p_dns_origin_name: String,
		domain_name: String,
	) -> Result<Self, Error> {
		let http_client_builder = reqwest::Client::builder().timeout(Duration::from_secs(3));

		#[cfg(not(debug_assertions))]
		{
			http_client_builder = http_client_builder.https_only(true);
		}

		let cloud_p2p_relay_url = cloud_p2p_relay_url
			.into_url()
			.map_err(Error::InvalidUrl)?
			.into();

		let cloud_p2p_dns_pkarr_url = cloud_p2p_dns_pkarr_url
			.into_url()
			.map_err(Error::InvalidUrl)?;

		let http_client =
			ClientBuilder::new(http_client_builder.build().map_err(Error::HttpClientInit)?)
				.with(RetryTransientMiddleware::new_with_policy(
					ExponentialBackoff::builder().build_with_max_retries(3),
				))
				.build();
		let get_cloud_api_address = get_cloud_api_address
			.into_url()
			.map_err(Error::InvalidUrl)?;

		let client_state = match Self::init_client(
			&http_client,
			get_cloud_api_address.clone(),
			domain_name.clone(),
		)
		.await
		{
			Ok(client) => Arc::new(RwLock::new(ClientState::Connected(client))),
			Err(e) => {
				warn!(
					?e,
					"Failed to initialize cloud services client; \
						This is a best effort and we will continue in Not Connected mode"
				);
				Arc::new(RwLock::new(ClientState::NotConnected))
			}
		};

		let (notify_user_tx, notify_user_rx) = flume::bounded(16);
		let (user_response_tx, user_response_rx) = flume::bounded(16);

		Ok(Self {
			client_state,
			token_refresher: TokenRefresher::new(
				http_client.clone(),
				get_cloud_api_address.clone(),
			),
			get_cloud_api_address,
			http_client,
			cloud_p2p_dns_origin_name,
			cloud_p2p_relay_url,
			cloud_p2p_dns_pkarr_url,
			domain_name,
			key_manager: Arc::default(),
			cloud_p2p: Arc::default(),
			notify_user_tx,
			notify_user_rx,
			user_response_tx,
			user_response_rx,
			has_bootstrapped: Arc::default(),
		})
	}

	pub fn stream_user_notifications(&self) -> impl Stream<Item = NotifyUser> + '_ {
		self.notify_user_rx.stream()
	}

	#[must_use]
	pub const fn http_client(&self) -> &ClientWithMiddleware {
		&self.http_client
	}

	/// Send back a user response to the Cloud P2P actor
	///
	/// # Panics
	/// Will panic if the channel is closed, which should never happen
	pub async fn send_user_response(&self, response: UserResponse) {
		self.user_response_tx
			.send_async(response)
			.await
			.expect("user response channel must never close");
	}

	async fn init_client(
		http_client: &ClientWithMiddleware,
		get_cloud_api_address: Url,
		domain_name: String,
	) -> Result<Client<QuinnConnection<Response, Request>>, Error> {
		let cloud_api_address = http_client
			.get(get_cloud_api_address)
			.send()
			.await
			.map_err(Error::FailedToRequestApiAddress)?
			.error_for_status()
			.map_err(Error::AuthServerError)?
			.text()
			.await
			.map_err(Error::FailedToExtractApiAddress)?
			.parse::<SocketAddr>()?;

		let mut crypto_config = {
			#[cfg(debug_assertions)]
			{
				#[derive(Debug)]
				struct SkipServerVerification;
				impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
					fn verify_server_cert(
						&self,
						_end_entity: &rustls::pki_types::CertificateDer<'_>,
						_intermediates: &[rustls::pki_types::CertificateDer<'_>],
						_server_name: &rustls::pki_types::ServerName<'_>,
						_ocsp_response: &[u8],
						_now: rustls::pki_types::UnixTime,
					) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
						Ok(rustls::client::danger::ServerCertVerified::assertion())
					}

					fn verify_tls12_signature(
						&self,
						_message: &[u8],
						_cert: &rustls::pki_types::CertificateDer<'_>,
						_dss: &rustls::DigitallySignedStruct,
					) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
					{
						Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
					}

					fn verify_tls13_signature(
						&self,
						_message: &[u8],
						_cert: &rustls::pki_types::CertificateDer<'_>,
						_dss: &rustls::DigitallySignedStruct,
					) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
					{
						Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
					}

					fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
						vec![
							rustls::SignatureScheme::RSA_PKCS1_SHA1,
							rustls::SignatureScheme::ECDSA_SHA1_Legacy,
							rustls::SignatureScheme::RSA_PKCS1_SHA256,
							rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
							rustls::SignatureScheme::RSA_PKCS1_SHA384,
							rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
							rustls::SignatureScheme::RSA_PKCS1_SHA512,
							rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
							rustls::SignatureScheme::RSA_PSS_SHA256,
							rustls::SignatureScheme::RSA_PSS_SHA384,
							rustls::SignatureScheme::RSA_PSS_SHA512,
							rustls::SignatureScheme::ED25519,
							rustls::SignatureScheme::ED448,
						]
					}
				}

				rustls::ClientConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
					.dangerous()
					.with_custom_certificate_verifier(Arc::new(SkipServerVerification))
					.with_no_client_auth()
			}

			#[cfg(not(debug_assertions))]
			{
				rustls::ClientConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
					.dangerous()
					.with_custom_certificate_verifier(Arc::new(
						rustls_platform_verifier::Verifier::new(),
					))
					.with_no_client_auth()
			}
		};

		crypto_config
			.alpn_protocols
			.extend([ServicesALPN::LATEST.to_vec()]);

		let client_config = ClientConfig::new(Arc::new(
			QuicClientConfig::try_from(crypto_config)
				.expect("misconfigured TLS client config, this is a bug and should crash"),
		));

		let mut endpoint = Endpoint::client("[::]:0".parse().expect("hardcoded address"))
			.map_err(Error::FailedToCreateEndpoint)?;
		endpoint.set_default_client_config(client_config);

		Ok(Client::new(RpcClient::new(QuinnConnection::new(
			endpoint,
			cloud_api_address,
			domain_name,
		))))
	}

	/// Returns a client to the cloud services.
	///
	/// If the client is not connected, it will try to connect to the cloud services.
	/// Available routes documented in
	/// [`sd_cloud_schema::Service`](https://github.com/spacedriveapp/cloud-services-schema).
	pub async fn client(&self) -> Result<Client<QuinnConnection<Response, Request>>, Error> {
		if let ClientState::Connected(client) = { &*self.client_state.read().await } {
			return Ok(client.clone());
		}

		// If we're not connected, we need to try to connect.
		let client = Self::init_client(
			&self.http_client,
			self.get_cloud_api_address.clone(),
			self.domain_name.clone(),
		)
		.await?;
		*self.client_state.write().await = ClientState::Connected(client.clone());

		Ok(client)
	}

	pub async fn set_key_manager(&self, key_manager: KeyManager) {
		self.key_manager
			.write()
			.await
			.replace(Arc::new(key_manager));
	}

	pub async fn key_manager(&self) -> Result<Arc<KeyManager>, Error> {
		self.key_manager
			.read()
			.await
			.as_ref()
			.map_or(Err(Error::KeyManagerNotInitialized), |key_manager| {
				Ok(Arc::clone(key_manager))
			})
	}

	pub async fn set_cloud_p2p(&self, cloud_p2p: CloudP2P) {
		self.cloud_p2p.write().await.replace(Arc::new(cloud_p2p));
	}

	pub async fn cloud_p2p(&self) -> Result<Arc<CloudP2P>, Error> {
		self.cloud_p2p
			.read()
			.await
			.as_ref()
			.map_or(Err(Error::CloudP2PNotInitialized), |cloud_p2p| {
				Ok(Arc::clone(cloud_p2p))
			})
	}
}

#[cfg(test)]
mod tests {
	use sd_cloud_schema::{auth, devices};

	use super::*;

	#[ignore]
	#[tokio::test]
	async fn test_client() {
		let response = CloudServices::new(
			"http://localhost:9420/cloud-api-address",
			"http://relay.localhost:9999/",
			"http://pkarr.localhost:9999/",
			"dns.localhost:9999".to_string(),
			"localhost".to_string(),
		)
		.await
		.unwrap()
		.client()
		.await
		.unwrap()
		.devices()
		.list(devices::list::Request {
			access_token: auth::AccessToken("invalid".to_string()),
		})
		.await
		.unwrap();

		assert!(matches!(
			response,
			Err(sd_cloud_schema::Error::Client(
				sd_cloud_schema::error::ClientSideError::Unauthorized
			))
		));
	}
}
