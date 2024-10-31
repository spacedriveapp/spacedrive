use sd_cloud_schema::auth::{AccessToken, RefreshToken};

use std::{pin::pin, time::Duration};

use base64::prelude::{Engine, BASE64_URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use futures_concurrency::stream::Merge;
use reqwest::Url;
use reqwest_middleware::{reqwest::header, ClientWithMiddleware};
use tokio::{
	spawn,
	sync::oneshot,
	time::{interval, sleep, MissedTickBehavior},
};
use tokio_stream::wrappers::IntervalStream;
use tracing::{error, warn};

use super::{Error, GetTokenError};

const ONE_MINUTE: Duration = Duration::from_secs(60);
const TEN_SECONDS: Duration = Duration::from_secs(10);

enum Message {
	Init(
		(
			AccessToken,
			RefreshToken,
			oneshot::Sender<Result<(), Error>>,
		),
	),
	CheckInitialization(oneshot::Sender<Result<(), GetTokenError>>),
	RequestToken(oneshot::Sender<Result<AccessToken, GetTokenError>>),
	RefreshTime,
	Tick,
}

#[derive(Debug, Clone)]
pub struct TokenRefresher {
	tx: flume::Sender<Message>,
}

impl TokenRefresher {
	pub(crate) fn new(http_client: ClientWithMiddleware, auth_server_url: Url) -> Self {
		let (tx, rx) = flume::bounded(8);

		spawn(async move {
			let refresh_url = auth_server_url
				.join("/api/auth/session/refresh")
				.expect("hardcoded refresh url path");

			while let Err(e) = spawn(Runner::run(
				http_client.clone(),
				refresh_url.clone(),
				rx.clone(),
			))
			.await
			{
				if e.is_panic() {
					if let Some(msg) = e.into_panic().downcast_ref::<&str>() {
						error!(?msg, "Panic in request handler!");
					} else {
						error!("Some unknown panic in request handler!");
					}
				}
			}
		});

		Self { tx }
	}

	pub async fn init(
		&self,
		access_token: AccessToken,
		refresh_token: RefreshToken,
	) -> Result<(), Error> {
		let (tx, rx) = oneshot::channel();
		self.tx
			.send_async(Message::Init((access_token, refresh_token, tx)))
			.await
			.expect("Token refresher channel closed");

		rx.await.expect("Token refresher channel closed")
	}

	pub async fn check_initialization(&self) -> Result<(), GetTokenError> {
		let (tx, rx) = oneshot::channel();
		self.tx
			.send_async(Message::CheckInitialization(tx))
			.await
			.expect("Token refresher channel closed");

		rx.await.expect("Token refresher channel closed")
	}

	pub async fn get_access_token(&self) -> Result<AccessToken, GetTokenError> {
		let (tx, rx) = oneshot::channel();
		self.tx
			.send_async(Message::RequestToken(tx))
			.await
			.expect("Token refresher channel closed");

		rx.await.expect("Token refresher channel closed")
	}
}

struct Runner {
	initialized: bool,
	http_client: ClientWithMiddleware,
	refresh_url: Url,
	current_token: Option<AccessToken>,
	current_refresh_token: Option<RefreshToken>,
	token_decoding_buffer: Vec<u8>,
	refresh_tx: flume::Sender<Message>,
}

impl Runner {
	async fn run(
		http_client: ClientWithMiddleware,
		refresh_url: Url,
		msgs_rx: flume::Receiver<Message>,
	) {
		let (refresh_tx, refresh_rx) = flume::bounded(1);

		let mut ticker = interval(TEN_SECONDS);
		ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

		let mut msg_stream = pin!((
			msgs_rx.into_stream(),
			refresh_rx.into_stream(),
			IntervalStream::new(ticker).map(|_| Message::Tick)
		)
			.merge());

		let mut runner = Self {
			initialized: false,
			http_client,
			refresh_url,
			current_token: None,
			current_refresh_token: None,
			token_decoding_buffer: Vec::new(),
			refresh_tx,
		};

		while let Some(msg) = msg_stream.next().await {
			match msg {
				Message::Init((access_token, refresh_token, ack)) => {
					if ack
						.send(runner.init(access_token, refresh_token).await)
						.is_err()
					{
						error!("Failed to send init token refresher response, receiver dropped;");
					}
				}

				Message::CheckInitialization(ack) => runner.check_initialization(ack),

				Message::RequestToken(ack) => runner.reply_token(ack),

				Message::RefreshTime => {
					if let Err(e) = runner.refresh().await {
						error!(?e, "Failed to refresh token: {e}");
					}
				}

				Message::Tick => runner.tick().await,
			}
		}
	}

	async fn init(
		&mut self,
		access_token: AccessToken,
		refresh_token: RefreshToken,
	) -> Result<(), Error> {
		let access_token_duration =
			Self::extract_access_token_duration(&mut self.token_decoding_buffer, &access_token)?;

		self.initialized = true;
		self.current_token = Some(access_token);
		self.current_refresh_token = Some(refresh_token);

		// If the token has an expiration smaller than a minute, we need to refresh it immediately.
		if access_token_duration < ONE_MINUTE {
			self.refresh_tx
				.send_async(Message::RefreshTime)
				.await
				.expect("refresh channel never closes");
		} else {
			// This task will be mostly parked waiting a sleep
			spawn(Self::schedule_refresh(
				self.refresh_tx.clone(),
				access_token_duration - ONE_MINUTE,
			));
		}

		Ok(())
	}

	fn reply_token(&self, ack: oneshot::Sender<Result<AccessToken, GetTokenError>>) {
		if ack
			.send(self.current_token.clone().ok_or({
				if self.initialized {
					GetTokenError::FailedToRefresh
				} else {
					GetTokenError::RefresherNotInitialized
				}
			}))
			.is_err()
		{
			warn!("Failed to send access token response, receiver dropped;");
		}
	}

	async fn refresh(&mut self) -> Result<(), Error> {
		let RefreshToken(refresh_token) = self
			.current_refresh_token
			.clone()
			.expect("refresh token is set otherwise we wouldn't be here");

		let response = self
			.http_client
			.post(self.refresh_url.clone())
			.header("rid", "session")
			.header(header::AUTHORIZATION, format!("Bearer {refresh_token}"))
			.send()
			.await
			.map_err(Error::RefreshTokenRequest)?
			.error_for_status()
			.map_err(Error::AuthServerError)?;

		if let (Some(access_token), Some(refresh_token)) = (
			response.headers().get("st-access-token"),
			response.headers().get("st-refresh-token"),
		) {
			// Only set values if we can parse both of them to strings
			let (access_token, refresh_token) = (
				Self::token_header_value_to_string(access_token)?,
				Self::token_header_value_to_string(refresh_token)?,
			);

			self.current_token = Some(AccessToken(access_token));
			self.current_refresh_token = Some(RefreshToken(refresh_token));
		} else {
			return Err(Error::MissingTokensOnRefreshResponse);
		}

		Ok(())
	}

	fn extract_access_token_duration(
		token_decoding_buffer: &mut Vec<u8>,
		AccessToken(token): &AccessToken,
	) -> Result<Duration, Error> {
		#[derive(serde::Deserialize)]
		struct Token {
			#[serde(with = "chrono::serde::ts_seconds")]
			exp: DateTime<Utc>,
		}

		token_decoding_buffer.clear();

		// The format of a JWT token is simple:
		// "<base64-encoded header>.<base64-encoded claims>.<signature>"
		BASE64_URL_SAFE_NO_PAD.decode_vec(
			token.split('.').nth(1).ok_or(Error::MissingClaims)?,
			token_decoding_buffer,
		)?;

		serde_json::from_slice::<Token>(token_decoding_buffer)?
			.exp
			.signed_duration_since(Utc::now())
			.to_std()
			.map_err(|_| Error::TokenExpired)
	}

	async fn schedule_refresh(refresh_tx: flume::Sender<Message>, wait_time: Duration) {
		sleep(wait_time).await;
		refresh_tx
			.send_async(Message::RefreshTime)
			.await
			.expect("Refresh channel closed");
	}

	fn token_header_value_to_string(token: &header::HeaderValue) -> Result<String, Error> {
		token.to_str().map(str::to_string).map_err(Into::into)
	}

	fn check_initialization(&self, ack: oneshot::Sender<Result<(), GetTokenError>>) {
		if ack
			.send(if self.initialized {
				Ok(())
			} else {
				Err(GetTokenError::RefresherNotInitialized)
			})
			.is_err()
		{
			warn!("Failed to send access token response, receiver dropped;");
		}
	}

	/// This method is a safeguard to make sure we try to keep refreshing tokens even if they
	/// already expired, as the refresh token has a bigger expiration than the access token.
	async fn tick(&mut self) {
		if let Some(access_token) = &self.current_token {
			if matches!(
				Self::extract_access_token_duration(&mut self.token_decoding_buffer, access_token),
				Err(Error::TokenExpired)
			) {
				if let Err(e) = self.refresh().await {
					error!(?e, "Failed to refresh expired token on tick method;");
				}
			}
		}
	}
}

/// This test is here for documentation purposes only, they are not meant to be run.
/// They're just examples of how to sign-up/sign-in and refresh tokens
#[cfg(test)]
mod tests {
	use reqwest::header;
	use reqwest_middleware::ClientBuilder;
	use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
	use serde_json::json;

	use crate::AUTH_SERVER_URL;

	use super::*;

	async fn get_tokens() -> (AccessToken, RefreshToken) {
		let client = reqwest::Client::new();

		let req_body = json!({
		  "formFields": [
			{
			  "id": "email",
			  "value": "johndoe@gmail.com"
			},
			{
			  "id": "password",
			  "value": "testPass123"
			}
		  ]
		});

		let response = client
			.post(format!("{AUTH_SERVER_URL}/api/auth/public/signup"))
			.header("rid", "emailpassword")
			.header("st-auth-mode", "header")
			.json(&req_body)
			.send()
			.await
			.unwrap();

		assert_eq!(response.status(), 200);

		if let (Some(access_token), Some(refresh_token)) = (
			response.headers().get("st-access-token"),
			response.headers().get("st-refresh-token"),
		) {
			(
				AccessToken(access_token.to_str().unwrap().to_string()),
				RefreshToken(refresh_token.to_str().unwrap().to_string()),
			)
		} else {
			let response = client
				.post(format!("{AUTH_SERVER_URL}/api/auth/public/signin"))
				.header("rid", "emailpassword")
				.header("st-auth-mode", "header")
				.json(&req_body)
				.send()
				.await
				.unwrap();

			assert_eq!(response.status(), 200);

			(
				AccessToken(
					response
						.headers()
						.get("st-access-token")
						.unwrap()
						.to_str()
						.unwrap()
						.to_string(),
				),
				RefreshToken(
					response
						.headers()
						.get("st-refresh-token")
						.unwrap()
						.to_str()
						.unwrap()
						.to_string(),
				),
			)
		}
	}

	#[ignore = "Documentation only"]
	#[tokio::test]
	async fn test_refresh_token() {
		let (AccessToken(access_token), RefreshToken(refresh_token)) = get_tokens().await;

		let client = reqwest::Client::new();
		let response = client
			.post(format!("{AUTH_SERVER_URL}/api/auth/session/refresh"))
			.header("rid", "session")
			.header(header::AUTHORIZATION, format!("Bearer {refresh_token}"))
			.send()
			.await
			.unwrap();

		assert_eq!(response.status(), 200);

		assert_ne!(
			response
				.headers()
				.get("st-access-token")
				.unwrap()
				.to_str()
				.unwrap(),
			access_token.as_str()
		);

		assert_ne!(
			response
				.headers()
				.get("st-refresh-token")
				.unwrap()
				.to_str()
				.unwrap(),
			refresh_token.as_str()
		);
	}

	#[ignore = "Needs an actual SuperTokens auth server running"]
	#[tokio::test]
	async fn test_refresher_runner() {
		let http_client_builder = reqwest::Client::builder().timeout(Duration::from_secs(3));

		let http_client = ClientBuilder::new(http_client_builder.build().unwrap())
			.with(RetryTransientMiddleware::new_with_policy(
				ExponentialBackoff::builder().build_with_max_retries(3),
			))
			.build();

		let (refresh_tx, _refresh_rx) = flume::bounded(1);

		let mut runner = Runner {
			initialized: false,
			http_client,
			refresh_url: Url::parse(&format!("{AUTH_SERVER_URL}/api/auth/session/refresh"))
				.unwrap(),
			current_token: None,
			current_refresh_token: None,
			token_decoding_buffer: Vec::new(),
			refresh_tx,
		};

		let (access_token, refresh_token) = get_tokens().await;

		runner.init(access_token, refresh_token).await.unwrap();

		runner.refresh().await.unwrap();
	}
}
