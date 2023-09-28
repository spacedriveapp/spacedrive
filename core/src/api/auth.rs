use std::time::Duration;

use reqwest::StatusCode;
use rspc::alpha::AlphaRouter;

use serde::Deserialize;
use serde::Serialize;
use specta::Type;

use crate::auth::{OAuthToken, DEVICE_CODE_URN};

use super::{Ctx, R};

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

					let auth_response: DeviceAuthorizationResponse = node.http
						.post(&format!("{}/login/device/code", &node.env.api_url))
						.send()
						.await
						.unwrap()
						.json()
						.await
						.unwrap();

					yield Response::Start {
						user_code: auth_response.user_code.clone(),
						verification_url: auth_response.verification_url.clone(),
						verification_url_complete: auth_response.verification_uri_complete.clone(),
					};

					yield loop {
						tokio::time::sleep(Duration::from_secs(5)).await;

						let token_resp = node.http
							.post(&format!("{}/login/oauth/access_token", &node.env.api_url))
							.form(&[("grant_type", DEVICE_CODE_URN), ("device_code", &auth_response.device_code)])
							.send()
							.await
							.unwrap();

						match token_resp.status() {
							StatusCode::OK => {
								let token: OAuthToken = token_resp.json().await.unwrap();

								node.config.write(|mut c| c.auth_token = Some(token)).await.ok();

								break Response::Complete;

							},
							StatusCode::BAD_REQUEST => {
								#[derive(Debug, Deserialize)]
								struct OAuth400 {
									error: String
								}

								let resp: OAuth400 = token_resp.json().await.unwrap();

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
				node.config.write(|mut c| c.auth_token = None).await.ok();

				Ok(())
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

				let res: Response = node
					.http
					.get(&format!("{}/api/v1/user/me", &node.env.api_url))
					.header("authorization", &auth_token.to_header())
					.send()
					.await
					.unwrap()
					.json()
					.await
					.unwrap();

				return Ok(res);
			})
		})
}
