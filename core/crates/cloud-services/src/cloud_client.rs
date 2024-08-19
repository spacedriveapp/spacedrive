use sd_cloud_schema::{Client, Service};

use std::{net::SocketAddr, sync::Arc, time::Duration};

use quic_rpc::{transport::quinn::QuinnConnection, RpcClient};
use quinn::{ClientConfig, Endpoint};
use reqwest::{IntoUrl, Url};
use reqwest_middleware::{reqwest, ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use tokio::sync::RwLock;
use tracing::warn;

use super::{error::Error, key_manager::KeyManager, token_refresher::TokenRefresher};

#[derive(Debug, Default)]
enum ClientState {
	#[default]
	NotConnected,
	Connected(Client<QuinnConnection<Service>, Service>),
}

/// Cloud services are a optional feature that allows you to interact with the cloud services
/// of Spacedrive.
/// They're optional in two different ways:
/// - The cloud services depends on a user being logged in with our server.
/// - The user being connected to the internet to begin with.
///
/// As we don't want to force the user to be connected to the internet, we have to make sure
/// that core can always operate without the cloud services.
#[derive(Debug, Clone)]
pub struct CloudServices {
	client_state: Arc<RwLock<ClientState>>,
	get_cloud_api_address: Url,
	http_client: ClientWithMiddleware,
	domain_name: String,
	pub token_refresher: TokenRefresher,
	pub key_manager: Option<Arc<KeyManager>>,
}

impl CloudServices {
	/// Creates a new cloud services client that can be used to interact with the cloud services.
	/// The client will try to connect to the cloud services on a best effort basis, as the user
	/// might not be connected to the internet.
	/// If the client fails to connect, it will try again the next time it's used.
	pub async fn new(
		get_cloud_api_address: impl IntoUrl + Send,
		domain_name: String,
	) -> Result<Self, Error> {
		let http_client_builder = reqwest::Client::builder().timeout(Duration::from_secs(3));

		#[cfg(not(debug_assertions))]
		{
			builder = builder.https_only(true);
		}

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

		Ok(Self {
			client_state,
			token_refresher: TokenRefresher::new(
				http_client.clone(),
				get_cloud_api_address.clone(),
			),
			get_cloud_api_address,
			http_client,
			domain_name,
			key_manager: None,
		})
	}

	async fn init_client(
		http_client: &ClientWithMiddleware,
		get_cloud_api_address: Url,
		domain_name: String,
	) -> Result<Client<QuinnConnection<Service>, Service>, Error> {
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

		let crypto_config = {
			#[cfg(debug_assertions)]
			{
				struct SkipServerVerification;
				impl rustls_old::client::ServerCertVerifier for SkipServerVerification {
					fn verify_server_cert(
						&self,
						_end_entity: &rustls_old::Certificate,
						_intermediates: &[rustls_old::Certificate],
						_server_name: &rustls_old::ServerName,
						_scts: &mut dyn Iterator<Item = &[u8]>,
						_ocsp_response: &[u8],
						_now: std::time::SystemTime,
					) -> Result<rustls_old::client::ServerCertVerified, rustls_old::Error> {
						Ok(rustls_old::client::ServerCertVerified::assertion())
					}
				}

				rustls_old::ClientConfig::builder()
					.with_safe_defaults()
					.with_custom_certificate_verifier(Arc::new(SkipServerVerification))
					.with_no_client_auth()
			}

			#[cfg(not(debug_assertions))]
			{
				rustls_old::ClientConfig::builder()
					.with_safe_defaults()
					.with_no_client_auth()
			}
		};

		let client_config = ClientConfig::new(Arc::new(crypto_config));

		let mut endpoint = Endpoint::client("[::]:0".parse().expect("hardcoded address"))
			.map_err(Error::FailedToCreateEndpoint)?;
		endpoint.set_default_client_config(client_config);

		// TODO(@fogodev): It's possible that we can't keep the connection alive all the time,
		// and need to use single shot connections. I will only be sure when we have
		// actually battle-tested the cloud services in core.
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
	pub async fn client(&self) -> Result<Client<QuinnConnection<Service>, Service>, Error> {
		if let ClientState::Connected(client) = &*self.client_state.read().await {
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
}

#[cfg(test)]
mod tests {
	use sd_cloud_schema::{auth, devices};

	use super::*;

	#[tokio::test]
	async fn test_client() {
		let response = CloudServices::new(
			"http://localhost:9420/cloud-api-address",
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
