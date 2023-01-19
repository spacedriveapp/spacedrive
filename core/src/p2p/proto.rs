use sd_crypto::{
	crypto::stream::Algorithm,
	keys::hashing::{HashingAlgorithm, Params},
	primitives::OnboardingConfig,
	Protected,
};
use sd_sync::CRDTOperation;
use serde::{Deserialize, Serialize};
use tracing::debug;
use uuid::Uuid;

use crate::{
	invalidate_query,
	library::{LibraryConfig, LibraryManager},
};

/// a request to another node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
	/// Ping a node to test the connection
	Ping,
	/// Sync operations to be applied to a library
	CRDTOperation((Uuid, Vec<CRDTOperation>, LibraryConfig)), // TODO: Remove `LibraryConfig` and do a proper pairing process once auth is merged in
}

/// a response from the request to another node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
	/// Don't send a response
	None,
	/// Acknowledge a ping
	Pong,
}

impl Request {
	pub async fn handle(self, library_manager: &LibraryManager) -> Result<Response, ()> {
		match self {
			Request::Ping => {
				debug!("Received ping!"); // TODO: Remove
				Ok(Response::Pong)
			}
			Request::CRDTOperation((library_id, ops, library_cfg)) => {
				debug!(
					"P2P Received Sync Operations for library '{}': {:?}",
					library_id, ops
				); // TODO: Remove

				// TODO: Break this code out into a proper pairing process once auth is merged in
				let ctx = match library_manager.get_ctx(library_id).await {
					Some(ctx) => ctx,
					None => {
						// TODO: Break this code out into a custom pairing routine once auth is merged in and make this case throw an error back to the other client

						library_manager
							.create(
								library_cfg,
								// TODO: Don't hardcode the `OnboardingConfig`
								OnboardingConfig {
									password: Protected::new("password".to_string()),
									secret_key: None,
									algorithm: Algorithm::XChaCha20Poly1305,
									hashing_algorithm: HashingAlgorithm::Argon2id(Params::Standard),
								},
							)
							.await
							.unwrap();

						library_manager
							.get_ctx(library_id)
							.await
							.expect("unreachable")
					}
				};

				// TODO: This should be done in a DB batch transaction by Brendan's sync system
				for op in ops {
					ctx.sync.ingest_op(op).await.unwrap();
				}

				invalidate_query!(ctx, "locations.list"); // TODO: Brendan's sync system needs to handle data invalidation

				Ok(Response::None)
			}
		}
	}
}
