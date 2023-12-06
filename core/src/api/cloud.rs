use base64::prelude::*;
use reqwest::Response;
use rspc::alpha::AlphaRouter;
use sd_prisma::prisma::instance;
use sd_utils::uuid_to_bytes;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use uuid::Uuid;

use crate::{invalidate_query, library::LibraryName};

use crate::util::http::ensure_response;

use super::{utils::library, Ctx, R};

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
}

mod library {
	use chrono::Utc;

	use crate::api::libraries::LibraryConfigWrapped;

	use super::*;

	#[derive(Serialize, Deserialize, Type)]
	#[specta(inline)]
	#[serde(rename_all = "camelCase")]
	struct Response {
		// id: String,
		uuid: Uuid,
		name: String,
		owner_id: String,
		instances: Vec<Instance>,
	}

	#[derive(Serialize, Deserialize, Type)]
	#[specta(inline)]
	#[serde(rename_all = "camelCase")]
	struct Instance {
		id: String,
		uuid: Uuid,
		identity: String,
	}

	pub fn mount() -> AlphaRouter<Ctx> {
		R.router()
			.procedure("get", {
				R.with2(library())
					.query(|(node, library), _: ()| async move {
						let library_id = library.id;
						let api_url = &node.env.api_url;

						node.authed_api_request(
							node.http
								.get(&format!("{api_url}/api/v1/libraries/{library_id}")),
						)
						.await
						.and_then(ensure_response)
						.map(parse_json_body::<Option<Response>>)?
						.await
					})
			})
			.procedure("list", {
				#[derive(Serialize, Deserialize, Type)]
				#[specta(inline)]
				#[serde(rename_all = "camelCase")]
				struct Response {
					// id: String,
					uuid: Uuid,
					name: String,
					owner_id: String,
					instances: Vec<Instance>,
				}

				#[derive(Serialize, Deserialize, Type)]
				#[specta(inline)]
				#[serde(rename_all = "camelCase")]
				struct Instance {
					id: String,
					uuid: Uuid,
				}

				R.query(|node, _: ()| async move {
					let api_url = &node.env.api_url;

					node.authed_api_request(node.http.get(&format!("{api_url}/api/v1/libraries")))
						.await
						.and_then(ensure_response)
						.map(parse_json_body::<Vec<Response>>)?
						.await
				})
			})
			.procedure("create", {
				R.with2(library())
					.mutation(|(node, library), _: ()| async move {
						let api_url = &node.env.api_url;
						let library_id = library.id;
						let instance_uuid = library.instance_uuid;

						node.authed_api_request(
							node.http
								.post(&format!("{api_url}/api/v1/libraries/{library_id}"))
								.json(&json!({
									"name": library.config().await.name,
									"instanceUuid": library.instance_uuid,
									"instanceIdentity": library.identity.to_remote_identity()
								})),
						)
						.await
						.and_then(ensure_response)?;

						invalidate_query!(library, "cloud.library.get");

						Ok(())
					})
			})
			.procedure("join", {
				R.mutation(|node, library_id: Uuid| async move {
					let api_url = &node.env.api_url;

					let Some(cloud_library) = node
						.authed_api_request(
							node.http
								.get(&format!("{api_url}/api/v1/libraries/{library_id}")),
						)
						.await
						.and_then(ensure_response)
						.map(parse_json_body::<Option<Response>>)?
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
							LibraryName::new(cloud_library.name).unwrap(),
							None,
							false,
							None,
							&node,
						)
						.await?;

					let instance_uuid = library.instance_uuid;

					node.authed_api_request(
						node.http
							.post(&format!(
								"{api_url}/api/v1/libraries/{library_id}/instances/{instance_uuid}"
							))
							.json(&json!({
								"instanceIdentity": library.identity.to_remote_identity()
							})),
					)
					.await
					.and_then(ensure_response)?;

					library
						.db
						.instance()
						.create_many(
							cloud_library
								.instances
								.into_iter()
								.map(|instance| {
									instance::create_unchecked(
										uuid_to_bytes(instance.uuid),
										BASE64_STANDARD.decode(instance.identity).unwrap(),
										vec![],
										"".to_string(),
										0,
										Utc::now().into(),
										Utc::now().into(),
										vec![],
									)
								})
								.collect(),
						)
						.exec()
						.await?;

					invalidate_query!(library, "cloud.library.get");

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
	use once_cell::sync::Lazy;
	use tokio::sync::Mutex;

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
				// TODO: Move this off a static. This is just for debugging.
				static AUTH_TOKEN: Lazy<Mutex<Option<AuthoriseResponse>>> =
					Lazy::new(|| Mutex::new(None));

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

				R.mutation(|node, id: String| async move {
					let token = {
						let mut token = AUTH_TOKEN.lock().await;
						if token.is_none() {
							let api_url = &node.env.api_url;

							*token = Some(
								node.authed_api_request(
									node.http
										.post(&format!("{api_url}/api/v1/locations/authorise"))
										.json(&json!({
											"id": id
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
						.key("test") // TODO: Proper access control to only the current locations files
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
