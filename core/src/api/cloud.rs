use crate::{invalidate_query, util::http::ensure_response};

use sd_prisma::prisma::instance;
use sd_utils::uuid_to_bytes;

use base64::prelude::*;
use reqwest::Response;
use rspc::alpha::AlphaRouter;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use uuid::Uuid;

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
	R.router().merge("library.", library::mount())
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
				R.with2(library())
					.mutation(|(node, library), _: ()| async move {
						let api_url = &node.env.api_url;
						let library_id = library.id;
						let instance_id = &library.instance_uuid;
						let db = &library.db;

						node.authed_api_request(
							node.http
								.post(&format!("{api_url}/api/v1/libraries/{library_id}/instances/{instance_id}"))
								.json(&json!({
									"instanceIdentity": library.identity.to_remote_identity()
								})),
						)
						.await
						.and_then(ensure_response)?;

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

						db._batch(
							cloud_library
								.instances
								.into_iter()
								.map(|instance| {
									db.instance().upsert(
										instance::pub_id::equals(uuid_to_bytes(instance.uuid)),
										instance::create(
											uuid_to_bytes(instance.uuid),
											BASE64_STANDARD
												.decode(instance.identity)
												.expect("failed to decode identity!"),
											vec![],
											"".to_string(),
											0,
											Utc::now().into(),
											Utc::now().into(),
											vec![],
										),
										vec![],
									)
								})
								.collect::<Vec<_>>(),
						)
						.await?;

						invalidate_query!(library, "cloud.library.get");
						invalidate_query!(library, "cloud.library.list");

						Ok(LibraryConfigWrapped::from_library(&library).await)
					})
			})
	}
}
