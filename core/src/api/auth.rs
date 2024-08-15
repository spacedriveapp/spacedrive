use std::time::Duration;

use reqwest::StatusCode;
use rspc::alpha::AlphaRouter;
use serde::{Deserialize, Serialize};
use specta::Type;

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
				Error(String),
			}

			R.subscription(|node, _: ()| async move {
				#[derive(Deserialize, Type)]
				struct DeviceAuthorizationResponse {
					device_code: String,
					user_code: String,
					verification_url: String,
					verification_uri_complete: String,
				}

				async_stream::stream! {
					let device_type = if cfg!(target_arch = "wasm32") {
						"web".to_string()
					} else if cfg!(target_os = "ios") || cfg!(target_os = "android") {
						"mobile".to_string()
					} else {
						"desktop".to_string()
					};

					let auth_response = match match node
						.http
						.post(&format!(
							"{}/login/device/code",
							&node.env.api_url.lock().await
						))
						.form(&[("client_id", &node.env.client_id), ("device", &device_type)])
						.send()
						.await
						.map_err(|e| e.to_string())
					{
						Ok(r) => r.json::<DeviceAuthorizationResponse>().await.map_err(|e| e.to_string()),
						Err(e) => {
							yield Response::Error(e.to_string());
							return
						},
					} {
						Ok(v) => v,
						Err(e) => {
							yield Response::Error(e.to_string());
							return
						},
					};

					yield Response::Start {
						user_code: auth_response.user_code.clone(),
						verification_url: auth_response.verification_url.clone(),
						verification_url_complete: auth_response.verification_uri_complete.clone(),
					};

					yield loop {
						tokio::time::sleep(Duration::from_secs(5)).await;

						let token_resp = match node.http
							.post(&format!("{}/login/oauth/access_token", &node.env.api_url.lock().await))
							.form(&[
								("grant_type", sd_cloud_api::auth::DEVICE_CODE_URN),
								("device_code", &auth_response.device_code),
								("client_id", &node.env.client_id)
							])
							.send()
							.await {
								Ok(v) => v,
								Err(e) => break Response::Error(e.to_string())
							};

						match token_resp.status() {
							StatusCode::OK => {
								let token = match token_resp.json().await {
									Ok(v) => v,
									Err(e) => break Response::Error(e.to_string())
								};

								if let Err(e) = node.config
									.write(|c| c.auth_token = Some(token))
									.await {
										break Response::Error(e.to_string());
									};


								break Response::Complete;
							},
							StatusCode::BAD_REQUEST => {
								#[derive(Debug, Deserialize)]
								struct OAuth400 {
									error: String
								}

								let resp = match token_resp.json::<OAuth400>().await {
									Ok(v) => v,
									Err(e) => break Response::Error(e.to_string())
								};

								match resp.error.as_str() {
									"authorization_pending" => continue,
									e => {
										break Response::Error(e.to_string())
									}
								}
							},
							s => {
								break Response::Error(s.to_string());
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
					.write(|c| c.auth_token = None)
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
				let resp = sd_cloud_api::user::me(node.cloud_api_config().await).await?;

				Ok(resp)
			})
		})
}
