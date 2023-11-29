use reqwest::Response;
use rspc::alpha::AlphaRouter;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use uuid::Uuid;

use crate::util::http::ensure_response;

use super::{utils::library, Ctx, R};

const ZERO_UUID: Uuid = Uuid::from_u128(0);

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
	use crate::{invalidate_query, library::LibraryName};

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
								.get(&format!("{api_url}/api/v1/libraries/{ZERO_UUID}")),
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
								.post(&format!("{api_url}/api/v1/libraries/{ZERO_UUID}"))
								.json(&json!({
									"name": library.config().await.name,
									"instanceUuid": library.instance_uuid
								})),
						)
						.await
						.and_then(ensure_response)?;

						invalidate_query!(library, "cloud.library.get");

						Ok(())
					})
			})
			.procedure("connect", {
				R.mutation(|node, library_id: Uuid| async move {
					let api_url = &node.env.api_url;

					let library = node
						.authed_api_request(
							node.http
								.get(&format!("{api_url}/api/v1/libraries/{library_id}")),
						)
						.await
						.and_then(ensure_response)
						.map(parse_json_body::<Option<Response>>)?
						.await;

					let library = node
						.libraries
						.create_with_uuid(
							library_id,
							LibraryName::new("Cloud Library".to_string()).unwrap(),
							None,
							false,
							None,
							&node,
						)
						.await?;

					let instance_uuid = library.instance_uuid;

					node.authed_api_request(node.http.post(&format!(
						"{api_url}/api/v1/libraries/{library_id}/instances/{instance_uuid}"
					)))
					.await
					.and_then(ensure_response)?;

					invalidate_query!(library, "cloud.library.get");

					Ok(())
				})
			})
	}
}
