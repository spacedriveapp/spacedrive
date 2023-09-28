use std::time::Duration;

use rspc::alpha::AlphaRouter;

use serde::Deserialize;
use serde::Serialize;
use specta::Type;

use super::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("loginSession", {
			#[derive(Serialize, Type)]
			enum Response {
				Start(String),
				Token(String),
			}

			R.subscription(|_, _: ()| async move {
				let device_session_url = format!(
					"{}/api/auth/device-session",
					std::env::var("SD_API_URL").unwrap()
				);

				let client = reqwest::Client::new();

				async_stream::stream! {
					let key = client
						.post(&device_session_url)
						.send()
						.await
						.unwrap()
						.text()
						.await
						.unwrap();

					yield Response::Start(key.clone());

					loop {
						tokio::time::sleep(Duration::from_secs(3)).await;

						#[derive(Debug, Deserialize)]
						#[serde(rename_all = "camelCase", tag = "status")]
						enum AuthResponse {
							Pending,
							Complete { token: String },
						}

						let result: AuthResponse = client
							.get(&device_session_url)
							.query(&[("key", &key)])
							.send()
							.await
							.unwrap()
							.json()
							.await
							.unwrap();

						if let AuthResponse::Complete { token } = result {
							yield Response::Token(token.clone());

							client
								.delete(&device_session_url)
								.query(&[("key", &key)])
								.send()
								.await
								// we don't care if this succeeds - redis will take care of it
								.ok();

							return;
						}
					}
				}
			})
		})
		.procedure("me", {
			R.query(|_, _: ()| async move {
				todo!();

				return Ok(());
			})
		})
}
