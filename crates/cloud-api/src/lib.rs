pub mod auth;

use auth::OAuthToken;
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use uuid::Uuid;

pub struct RequestConfig {
	pub client: reqwest::Client,
	pub api_url: String,
	pub auth_token: Option<auth::OAuthToken>,
}

#[derive(thiserror::Error, Debug)]
#[error("{0}")]
pub struct Error(String);

impl From<Error> for rspc::Error {
	fn from(e: Error) -> rspc::Error {
		rspc::Error::new(rspc::ErrorCode::InternalServerError, e.0)
	}
}

#[derive(Serialize, Deserialize, Debug, Type)]
#[serde(rename_all = "camelCase")]
#[specta(rename = "CloudLibrary")]
pub struct Library {
	pub uuid: Uuid,
	pub name: String,
	pub instances: Vec<Instance>,
	pub owner_id: String,
}

#[derive(Serialize, Deserialize, Debug, Type)]
#[serde(rename_all = "camelCase")]
#[specta(rename = "CloudInstance")]
pub struct Instance {
	pub id: String,
	pub uuid: Uuid,
	pub identity: String,
}

trait WithAuth {
	fn with_auth(self, token: OAuthToken) -> Self;
}

impl WithAuth for reqwest::RequestBuilder {
	fn with_auth(self, token: OAuthToken) -> Self {
		self.header(
			"authorization",
			format!("{} {}", token.token_type, token.access_token),
		)
	}
}

pub use library_get::exec as library_get;
pub mod library_get {
	use super::*;

	pub async fn exec(config: RequestConfig, library_id: Uuid) -> Result<Response, Error> {
		let Some(auth_token) = config.auth_token else {
			return Err(Error("Authentication required".to_string()));
		};

		Ok(config
			.client
			.get(&format!(
				"{}/api/v1/libraries/{}",
				config.api_url, library_id
			))
			.with_auth(auth_token)
			.send()
			.await
			.map_err(|e| Error(e.to_string()))?
			.json()
			.await
			.map_err(|e| Error(e.to_string()))?)
	}

	pub type Response = Option<Library>;
}

pub use library_list::exec as library_list;
pub mod library_list {
	use super::*;

	pub async fn exec(config: RequestConfig) -> Result<Response, Error> {
		let Some(auth_token) = config.auth_token else {
			return Err(Error("Authentication required".to_string()));
		};

		Ok(config
			.client
			.get(&format!("{}/api/v1/libraries", config.api_url))
			.with_auth(auth_token)
			.send()
			.await
			.map_err(|e| Error(e.to_string()))?
			.json()
			.await
			.map_err(|e| Error(e.to_string()))?)
	}

	pub type Response = Vec<Library>;
}

pub use library_create::exec as library_create;
pub mod library_create {
	use super::*;

	pub async fn exec(
		config: RequestConfig,
		library_id: Uuid,
		name: &str,
		instance_uuid: Uuid,
		instance_identity: &impl Serialize,
	) -> Result<(), Error> {
		let Some(auth_token) = config.auth_token else {
			return Err(Error("Authentication required".to_string()));
		};

		let resp = config
			.client
			.post(&format!(
				"{}/api/v1/libraries/{}",
				config.api_url, library_id
			))
			.json(&json!({
				"name":name,
				"instanceUuid": instance_uuid,
				"instanceIdentity": instance_identity
			}))
			.with_auth(auth_token)
			.send()
			.await
			.map_err(|e| Error(e.to_string()))?
			.text()
			.await
			.map_err(|e| Error(e.to_string()))?;

		println!("{resp}");

		Ok(())
	}
}

pub use library_join::exec as library_join;
pub mod library_join {
	use super::*;

	pub async fn exec(
		config: RequestConfig,
		library_id: Uuid,
		instance_uuid: Uuid,
		instance_identity: &impl Serialize,
	) -> Result<(), Error> {
		let Some(auth_token) = config.auth_token else {
			return Err(Error("Authentication required".to_string()));
		};

		let resp = config
			.client
			.post(&format!(
				"{}/api/v1/libraries/{library_id}/instances/{instance_uuid}",
				config.api_url
			))
			.json(&json!({ "instanceIdentity": instance_identity }))
			.with_auth(auth_token)
			.send()
			.await
			.map_err(|e| Error(e.to_string()))?
			.text()
			.await
			.map_err(|e| Error(e.to_string()))?;

		println!("{resp}");

		Ok(())
	}
}
