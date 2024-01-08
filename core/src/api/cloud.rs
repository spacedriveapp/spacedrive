use crate::{api::libraries::LibraryConfigWrapped, invalidate_query, library::LibraryName};

use reqwest::Response;
use rspc::alpha::AlphaRouter;
use serde::de::DeserializeOwned;

use uuid::Uuid;

use super::{utils::library, Ctx, R};

#[allow(unused)]
async fn parse_json_body<T: DeserializeOwned>(response: Response) -> Result<T, rspc::Error> {
	response.json().await.map_err(|_| {
		rspc::Error::new(
			rspc::ErrorCode::InternalServerError,
			"JSON conversion failed".to_string(),
		)
	})
}

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.merge("library.", library::mount())
		.merge("locations.", locations::mount())
		.procedure("getApiOrigin", {
			R.query(|node, _: ()| async move { Ok(node.env.api_url.lock().await.to_string()) })
		})
		.procedure("setApiOrigin", {
			R.mutation(|node, origin: String| async move {
				let mut origin_env = node.env.api_url.lock().await;
				*origin_env = origin;

				node.config.write(|c| c.auth_token = None).await.ok();

				Ok(())
			})
		})
}

mod library {

	use super::*;

	pub fn mount() -> AlphaRouter<Ctx> {
		R.router()
			.procedure("get", {
				R.with2(library())
					.query(|(node, library), _: ()| async move {
						Ok(
							sd_cloud_api::library::get(node.cloud_api_config().await, library.id)
								.await?,
						)
					})
			})
			.procedure("list", {
				R.query(|node, _: ()| async move {
					Ok(sd_cloud_api::library::list(node.cloud_api_config().await).await?)
				})
			})
			.procedure("create", {
				R.with2(library())
					.mutation(|(node, library), _: ()| async move {
						sd_cloud_api::library::create(
							node.cloud_api_config().await,
							library.id,
							&library.config().await.name,
							library.instance_uuid,
							&library.identity.to_remote_identity(),
						)
						.await?;

						invalidate_query!(library, "cloud.library.get");

						Ok(())
					})
			})
			.procedure("join", {
				R.mutation(|node, library_id: Uuid| async move {
					let Some(cloud_library) =
						sd_cloud_api::library::get(node.cloud_api_config().await, library_id)
							.await?
					else {
						return Err(rspc::Error::new(
							rspc::ErrorCode::NotFound,
							"Library not found".to_string(),
						));
					};

					let library = node
						.libraries
						.create_with_uuid(
							library_id,
							LibraryName::new(cloud_library.name).map_err(|e| {
								rspc::Error::new(
									rspc::ErrorCode::InternalServerError,
									e.to_string(),
								)
							})?,
							None,
							false,
							None,
							&node,
						)
						.await?;

					sd_cloud_api::library::join(
						node.cloud_api_config().await,
						library_id,
						library.instance_uuid,
						&library.identity.to_remote_identity(),
					)
					.await?;

					invalidate_query!(library, "cloud.library.get");
					invalidate_query!(library, "cloud.library.list");

					Ok(LibraryConfigWrapped::from_library(&library).await)
				})
			})
	}
}

mod locations {
	use aws_config::{Region, SdkConfig};
	use aws_credential_types::provider::future;
	use aws_sdk_s3::{
		config::{Credentials, ProvideCredentials, SharedCredentialsProvider},
		primitives::ByteStream,
	};
	use http_body::Full;
	use serde::{Deserialize, Serialize};
	use serde_json::json;
	use specta::Type;

	use crate::util::http::ensure_response;

	use super::*;

	#[derive(Type, Serialize, Deserialize)]
	pub struct CloudLocation {
		id: String,
		name: String,
	}

	#[derive(Debug, Clone, Type, Deserialize)]
	pub struct AuthoriseResponse {
		access_key_id: String,
		secret_access_key: String,
		session_token: String,
	}

	pub fn mount() -> AlphaRouter<Ctx> {
		R.router()
			.procedure("list", {
				R.query(|node, _: ()| async move {
					let api_url = &node.env.api_url;

					node.authed_api_request(node.http.get(&format!("{api_url}/api/v1/locations")))
						.await
						.and_then(ensure_response)
						.map(parse_json_body::<Vec<CloudLocation>>)?
						.await
				})
			})
			.procedure("create", {
				R.mutation(|node, name: String| async move {
					let api_url = &node.env.api_url;

					node.authed_api_request(
						node.http
							.post(&format!("{api_url}/api/v1/locations"))
							.json(&json!({
								"name": name
							})),
					)
					.await
					.and_then(ensure_response)
					.map(parse_json_body::<CloudLocation>)?
					.await
				})
			})
			.procedure("remove", {
				R.mutation(|node, id: String| async move {
					let api_url = &node.env.api_url;

					node.authed_api_request(
						node.http
							.post(&format!("{api_url}/api/v1/locations/delete"))
							.json(&json!({
								"id": id
							})),
					)
					.await
					.and_then(ensure_response)?;

					Ok(())
				})
			})
			// TODO: Remove this
			.procedure("testing", {
				// // TODO: Move this off a static. This is just for debugging.
				// static AUTH_TOKEN: Lazy<Mutex<Option<AuthoriseResponse>>> =
				// 	Lazy::new(|| Mutex::new(None));

				#[derive(Debug)]
				pub struct CredentialsProvider(AuthoriseResponse);

				impl ProvideCredentials for CredentialsProvider {
					fn provide_credentials<'a>(&'a self) -> future::ProvideCredentials<'a>
					where
						Self: 'a,
					{
						future::ProvideCredentials::ready(Ok(Credentials::new(
							self.0.access_key_id.clone(),
							self.0.secret_access_key.clone(),
							Some(self.0.session_token.clone()),
							None, // TODO: Get this from the SD Cloud backend
							"sd-cloud",
						)))
					}

					fn fallback_on_interrupt(&self) -> Option<Credentials> {
						None
					}
				}

				#[derive(Type, Deserialize)]
				pub struct TestingParams {
					id: String,
					path: String,
				}

				R.mutation(|node, params: TestingParams| async move {
					let token = {
						let token = &mut None; // AUTH_TOKEN.lock().await; // TODO: Caching of the token. For now it's annoying when debugging.
						if token.is_none() {
							let api_url = &node.env.api_url;

							*token = Some(
								node.authed_api_request(
									node.http
										.post(&format!("{api_url}/api/v1/locations/authorise"))
										.json(&json!({
											"id": params.id
										})),
								)
								.await
								.and_then(ensure_response)
								.map(parse_json_body::<AuthoriseResponse>)?
								.await?,
							);
						}

						token.clone().expect("Checked above")
					};

					println!("{token:?}"); // TODO

					// TODO: Reuse the client between procedure calls
					let client = aws_sdk_s3::Client::new(
						&SdkConfig::builder()
							.region(Region::new("us-west-1")) // TODO: From cloud config
							.credentials_provider(SharedCredentialsProvider::new(
								CredentialsProvider(token),
							))
							.build(),
					);

					client
						.put_object()
						.bucket("spacedrive-cloud") // TODO: From cloud config
						.key(params.path) // TODO: Proper access control to only the current locations files
						.body(ByteStream::from_body_0_4(Full::from("Hello, world!")))
						.send()
						.await
						.map_err(|err| {
							tracing::error!("S3 error: {err:?}");
							rspc::Error::new(
								rspc::ErrorCode::InternalServerError,
								"Failed to upload to S3".to_string(),
							)
						})?; // TODO: Error handling

					println!("Uploaded file!");

					Ok(())
				})
			})
	}
}
