use std::time::Duration;

use reqwest::RequestBuilder;
use reqwest::StatusCode;
use rspc::alpha::AlphaRouter;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;
use specta::Type;

use crate::auth::DEVICE_CODE_URN;

use super::{Ctx, R};

async fn json_req<T: DeserializeOwned>(req: RequestBuilder) -> Result<T, rspc::Error> {
	req.send()
		.await
		.map_err(|_| {
			rspc::Error::new(
				rspc::ErrorCode::InternalServerError,
				"Request failed".to_string(),
			)
		})?
		.json()
		.await
		.map_err(|_| {
			rspc::Error::new(
				rspc::ErrorCode::InternalServerError,
				"JSON conversion failed".to_string(),
			)
		})
}

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("loginSession", {
			#[derive(Serialize, Type)]
			#[specta(inline)]
			enum Response {
				Start {
					user_code: String,
					verification_url: String,
					verification_url_complete: String,
				},
				Complete,
				Error,
			}

			R.subscription(|node, _: ()| async move {
				async_stream::stream! {
					#[derive(Deserialize, Type)]
					struct DeviceAuthorizationResponse {
						device_code: String,
						user_code: String,
						verification_url: String,
						verification_uri_complete: String,
					}

					let Ok(auth_response) = (match node.http
						.post(&format!("{}/login/device/code", &node.env.api_url))
						.send()
						.await {
							Ok(resp) => resp.json::<DeviceAuthorizationResponse>().await,
							Err(e) => Err(e)
						}) else {
							yield Response::Error;
							return;
						};

					yield Response::Start {
						user_code: auth_response.user_code.clone(),
						verification_url: auth_response.verification_url.clone(),
						verification_url_complete: auth_response.verification_uri_complete.clone(),
					};

					yield loop {
						tokio::time::sleep(Duration::from_secs(5)).await;

						let Ok(token_resp) = node.http
							.post(&format!("{}/login/oauth/access_token", &node.env.api_url))
							.form(&[("grant_type", DEVICE_CODE_URN), ("device_code", &auth_response.device_code)])
							.send()
							.await else {
								break Response::Error;
							};

						match token_resp.status() {
							StatusCode::OK => {
								let Ok(token) = token_resp.json().await else {
									break Response::Error;
								};

								if node.config
									.write(|mut c| c.auth_token = Some(token))
									.await.is_err() {
										break Response::Error;
									};


								break Response::Complete;
							},
							StatusCode::BAD_REQUEST => {
								#[derive(Debug, Deserialize)]
								struct OAuth400 {
									error: String
								}

								let Ok(resp) = token_resp.json::<OAuth400>().await else {
									break Response::Error;
								};

								match resp.error.as_str() {
									"authorization_pending" => continue,
									_ => {
										break Response::Error;
									}
								}
							},
							_ => {
								break Response::Error;
							}
						}
					}
				}
			})
		})
		.procedure(
			"logout",
			R.mutation(|node, _: ()| async move {
				node.config
					.write(|mut c| c.auth_token = None)
					.await
					.map(|_| ())
					.map_err(|_| {
						rspc::Error::new(
							rspc::ErrorCode::InternalServerError,
							"Failed to write config".to_string(),
						)
					})
			}),
		)
		.procedure("me", {
			R.query(|node, _: ()| async move {
				let Some(auth_token) = node.config.get().await.auth_token else {
					return Err(rspc::Error::new(
						rspc::ErrorCode::Unauthorized,
						"No auth token".to_string(),
					));
				};

				#[derive(Serialize, Deserialize, Type)]
				#[specta(inline)]
				struct Response {
					id: String,
					email: String,
				}

				let res: Response = json_req(
					node.http
						.get(&format!("{}/api/v1/user/me", &node.env.api_url))
						.header("authorization", &auth_token.to_header()),
				)
				.await?;

				Ok(res)
			})
		})
}
