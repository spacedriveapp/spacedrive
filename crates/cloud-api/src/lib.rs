pub mod auth;

use std::{collections::HashMap, future::Future, sync::Arc};

use auth::OAuthToken;
use sd_p2p::RemoteIdentity;
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use uuid::Uuid;

pub struct RequestConfig {
	pub client: reqwest::Client,
	pub api_url: String,
	pub auth_token: Option<auth::OAuthToken>,
}

pub trait RequestConfigProvider {
	fn get_request_config(self: &Arc<Self>) -> impl Future<Output = RequestConfig> + Send;
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
	pub id: String,
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
	pub identity: RemoteIdentity,
	#[serde(rename = "nodeId")]
	pub node_id: Uuid,
	pub node_remote_identity: String,
	pub metadata: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Type)]
#[serde(rename_all = "camelCase")]
#[specta(rename = "CloudMessageCollection")]
pub struct MessageCollection {
	pub instance_uuid: Uuid,
	pub start_time: String,
	pub end_time: String,
	pub contents: String,
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

pub mod feedback {
	use super::*;

	pub use send::exec as send;
	pub mod send {
		use super::*;

		pub async fn exec(config: RequestConfig, message: String, emoji: u8) -> Result<(), Error> {
			let mut req = config
				.client
				.post(format!("{}/api/v1/feedback", config.api_url))
				.json(&json!({
					"message": message,
					"emoji": emoji,
				}));

			if let Some(auth_token) = config.auth_token {
				req = req.with_auth(auth_token);
			}

			req.send()
				.await
				.and_then(|r| r.error_for_status())
				.map_err(|e| Error(e.to_string()))?;

			Ok(())
		}
	}
}

pub mod user {
	use super::*;

	pub use me::exec as me;
	pub mod me {
		use super::*;

		#[derive(Serialize, Deserialize, Type)]
		#[specta(inline)]
		pub struct Response {
			id: String,
			email: String,
		}

		pub async fn exec(config: RequestConfig) -> Result<Response, Error> {
			let Some(auth_token) = config.auth_token else {
				return Err(Error("Authentication required".to_string()));
			};

			config
				.client
				.get(&format!("{}/api/v1/user/me", config.api_url))
				.with_auth(auth_token)
				.send()
				.await
				.map_err(|e| Error(e.to_string()))?
				.json()
				.await
				.map_err(|e| Error(e.to_string()))
		}
	}
}

pub mod library {
	use super::*;

	pub use get::exec as get;
	pub mod get {
		use super::*;

		pub async fn exec(config: RequestConfig, library_id: Uuid) -> Result<Response, Error> {
			let Some(auth_token) = config.auth_token else {
				return Err(Error("Authentication required".to_string()));
			};

			config
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
				.map_err(|e| Error(e.to_string()))
		}

		pub type Response = Option<Library>;
	}

	pub use list::exec as list;
	pub mod list {
		use super::*;

		pub async fn exec(config: RequestConfig) -> Result<Response, Error> {
			let Some(auth_token) = config.auth_token else {
				return Err(Error("Authentication required".to_string()));
			};

			config
				.client
				.get(&format!("{}/api/v1/libraries", config.api_url))
				.with_auth(auth_token)
				.send()
				.await
				.map_err(|e| Error(e.to_string()))?
				.json()
				.await
				.map_err(|e| Error(e.to_string()))
		}

		pub type Response = Vec<Library>;
	}

	pub use create::exec as create;
	pub mod create {
		use super::*;

		#[derive(Debug, Deserialize)]
		pub struct CreateResult {
			pub id: String,
		}

		#[allow(clippy::too_many_arguments)]
		pub async fn exec(
			config: RequestConfig,
			library_id: Uuid,
			name: &str,
			instance_uuid: Uuid,
			instance_identity: RemoteIdentity,
			node_id: Uuid,
			node_remote_identity: RemoteIdentity,
			metadata: &HashMap<String, String>,
		) -> Result<CreateResult, Error> {
			let Some(auth_token) = config.auth_token else {
				return Err(Error("Authentication required".to_string()));
			};

			config
				.client
				.post(&format!(
					"{}/api/v1/libraries/{}",
					config.api_url, library_id
				))
				.json(&json!({
					"name":name,
					"instanceUuid": instance_uuid,
					"instanceIdentity": instance_identity,
					"nodeId": node_id,
					"nodeRemoteIdentity": node_remote_identity,
					"metadata": metadata,
				}))
				.with_auth(auth_token)
				.send()
				.await
				.map_err(|e| Error(e.to_string()))?
				.json()
				.await
				.map_err(|e| Error(e.to_string()))
		}
	}

	pub use update::exec as update;
	pub mod update {
		use super::*;

		pub async fn exec(
			config: RequestConfig,
			library_id: Uuid,
			name: Option<String>,
		) -> Result<(), Error> {
			let Some(auth_token) = config.auth_token else {
				return Err(Error("Authentication required".to_string()));
			};

			config
				.client
				.patch(&format!(
					"{}/api/v1/libraries/{}",
					config.api_url, library_id
				))
				.json(&json!({
					"name":name
				}))
				.with_auth(auth_token)
				.send()
				.await
				.map_err(|e| Error(e.to_string()))
				.map(|_| ())
		}
	}

	pub use update_instance::exec as update_instance;
	pub mod update_instance {
		use super::*;

		pub async fn exec(
			config: RequestConfig,
			library_id: Uuid,
			instance_id: Uuid,
			node_id: Option<Uuid>,
			node_remote_identity: Option<RemoteIdentity>,
			metadata: Option<HashMap<String, String>>,
		) -> Result<(), Error> {
			let Some(auth_token) = config.auth_token else {
				return Err(Error("Authentication required".to_string()));
			};

			config
				.client
				.patch(&format!(
					"{}/api/v1/libraries/{}/{}",
					config.api_url, library_id, instance_id
				))
				.json(&json!({
					"nodeId": node_id,
					"nodeRemoteIdentity": node_remote_identity,
					"metadata": metadata,
				}))
				.with_auth(auth_token)
				.send()
				.await
				.map_err(|e| Error(e.to_string()))
				.map(|_| ())
		}
	}

	pub use join::exec as join;
	pub mod join {
		use super::*;

		pub async fn exec(
			config: RequestConfig,
			library_id: Uuid,
			instance_uuid: Uuid,
			instance_identity: RemoteIdentity,
			node_id: Uuid,
			node_remote_identity: RemoteIdentity,
			metadata: HashMap<String, String>,
		) -> Result<Vec<Instance>, Error> {
			let Some(auth_token) = config.auth_token else {
				return Err(Error("Authentication required".to_string()));
			};

			config
				.client
				.post(&format!(
					"{}/api/v1/libraries/{library_id}/instances/{instance_uuid}",
					config.api_url
				))
				.json(&json!({
					"instanceIdentity": instance_identity,
					"nodeId": node_id,
					"nodeRemoteIdentity": node_remote_identity,
					"metadata": metadata,
				}))
				.with_auth(auth_token)
				.send()
				.await
				.map_err(|e| Error(e.to_string()))?
				.json()
				.await
				.map_err(|e| Error(e.to_string()))
		}
	}

	pub mod message_collections {
		use super::*;

		pub use get::exec as get;
		pub mod get {
			use super::*;

			#[derive(Serialize)]
			#[serde(rename_all = "camelCase")]
			pub struct InstanceTimestamp {
				pub instance_uuid: Uuid,
				pub from_time: String,
			}

			pub async fn exec(
				config: RequestConfig,
				library_id: Uuid,
				this_instance_uuid: Uuid,
				timestamps: Vec<InstanceTimestamp>,
			) -> Result<Response, Error> {
				let Some(auth_token) = config.auth_token else {
					return Err(Error("Authentication required".to_string()));
				};

				config
					.client
					.post(&format!(
						"{}/api/v1/libraries/{}/messageCollections/get",
						config.api_url, library_id
					))
					.json(&json!({
						"instanceUuid": this_instance_uuid,
						"timestamps": timestamps
					}))
					.with_auth(auth_token)
					.send()
					.await
					.map_err(|e| Error(e.to_string()))?
					.json()
					.await
					.map_err(|e| Error(e.to_string()))
			}

			pub type Response = Vec<MessageCollection>;
		}

		pub use request_add::exec as request_add;
		pub mod request_add {
			use super::*;

			#[derive(Deserialize, Debug)]
			#[serde(rename_all = "camelCase")]
			pub struct RequestAdd {
				pub instance_uuid: Uuid,
				pub from_time: Option<String>,
				// mutex key on the instance
				pub key: String,
			}

			pub async fn exec(
				config: RequestConfig,
				library_id: Uuid,
				instances: Vec<Uuid>,
			) -> Result<Response, Error> {
				let Some(auth_token) = config.auth_token else {
					return Err(Error("Authentication required".to_string()));
				};

				let instances = instances
					.into_iter()
					.map(|i| json!({"instanceUuid": i }))
					.collect::<Vec<_>>();

				config
					.client
					.post(&format!(
						"{}/api/v1/libraries/{}/messageCollections/requestAdd",
						config.api_url, library_id
					))
					.json(&json!({ "instances": instances }))
					.with_auth(auth_token)
					.send()
					.await
					.map_err(|e| Error(e.to_string()))?
					.json()
					.await
					.map_err(|e| Error(e.to_string()))
			}

			pub type Response = Vec<RequestAdd>;
		}

		pub use do_add::exec as do_add;
		pub mod do_add {
			use super::*;

			#[derive(Serialize, Debug)]
			#[serde(rename_all = "camelCase")]
			pub struct Input {
				pub uuid: Uuid,
				pub key: String,
				pub start_time: String,
				pub end_time: String,
				pub contents: String,
				pub ops_count: usize,
			}

			pub async fn exec(
				config: RequestConfig,
				library_id: Uuid,
				instances: Vec<Input>,
			) -> Result<(), Error> {
				let Some(auth_token) = config.auth_token else {
					return Err(Error("Authentication required".to_string()));
				};

				config
					.client
					.post(&format!(
						"{}/api/v1/libraries/{}/messageCollections/doAdd",
						config.api_url, library_id
					))
					.json(&json!({ "instances": instances }))
					.with_auth(auth_token)
					.send()
					.await
					.and_then(|r| r.error_for_status())
					.map_err(|e| Error(e.to_string()))?;

				Ok(())
			}
		}
	}
}

#[derive(Type, Serialize, Deserialize)]
pub struct CloudLocation {
	id: String,
	name: String,
}

pub mod locations {
	use super::*;

	pub use list::exec as list;
	pub mod list {
		use super::*;

		pub async fn exec(config: RequestConfig) -> Result<Response, Error> {
			let Some(auth_token) = config.auth_token else {
				return Err(Error("Authentication required".to_string()));
			};

			config
				.client
				.get(&format!("{}/api/v1/locations", config.api_url))
				.with_auth(auth_token)
				.send()
				.await
				.map_err(|e| Error(e.to_string()))?
				.json()
				.await
				.map_err(|e| Error(e.to_string()))
		}

		pub type Response = Vec<CloudLocation>;
	}

	pub use create::exec as create;
	pub mod create {
		use super::*;

		pub async fn exec(config: RequestConfig, name: String) -> Result<Response, Error> {
			let Some(auth_token) = config.auth_token else {
				return Err(Error("Authentication required".to_string()));
			};

			config
				.client
				.post(&format!("{}/api/v1/locations", config.api_url))
				.json(&json!({
					"name": name,
				}))
				.with_auth(auth_token)
				.send()
				.await
				.map_err(|e| Error(e.to_string()))?
				.json()
				.await
				.map_err(|e| Error(e.to_string()))
		}

		pub type Response = CloudLocation;
	}

	pub use remove::exec as remove;
	pub mod remove {
		use super::*;

		pub async fn exec(config: RequestConfig, id: String) -> Result<Response, Error> {
			let Some(auth_token) = config.auth_token else {
				return Err(Error("Authentication required".to_string()));
			};

			config
				.client
				.post(&format!("{}/api/v1/locations/delete", config.api_url))
				.json(&json!({
					"id": id,
				}))
				.with_auth(auth_token)
				.send()
				.await
				.map_err(|e| Error(e.to_string()))?
				.json()
				.await
				.map_err(|e| Error(e.to_string()))
		}

		pub type Response = CloudLocation;
	}

	pub use authorize::exec as authorize;
	pub mod authorize {
		use super::*;

		pub async fn exec(config: RequestConfig, id: String) -> Result<Response, Error> {
			let Some(auth_token) = config.auth_token else {
				return Err(Error("Authentication required".to_string()));
			};

			config
				.client
				.post(&format!("{}/api/v1/locations/authorize", config.api_url))
				.json(&json!({ "id": id }))
				.with_auth(auth_token)
				.send()
				.await
				.map_err(|e| Error(e.to_string()))?
				.json()
				.await
				.map_err(|e| Error(e.to_string()))
		}

		#[derive(Debug, Clone, Type, Deserialize)]
		pub struct Response {
			pub access_key_id: String,
			pub secret_access_key: String,
			pub session_token: String,
		}
	}
}
