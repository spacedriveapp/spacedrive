use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::{input::VouchingSessionInput, output::VouchingSessionOutput};
use crate::infra::query::{CoreQuery, QueryError, QueryResult};
use crate::{context::CoreContext, service::network::protocol::PairingProtocolHandler};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VouchingSessionQuery {
	session_id: uuid::Uuid,
}

impl CoreQuery for VouchingSessionQuery {
	type Input = VouchingSessionInput;
	type Output = VouchingSessionOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self {
			session_id: input.session_id,
		})
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let net = context
			.get_networking()
			.await
			.ok_or_else(|| QueryError::Internal("Networking not initialized".to_string()))?;

		let registry = net.protocol_registry();
		let guard = registry.read().await;
		if let Some(handler) = guard.get_handler("pairing") {
			if let Some(pairing) = handler.as_any().downcast_ref::<PairingProtocolHandler>() {
				let session = pairing.get_vouching_session(self.session_id).await;
				return Ok(VouchingSessionOutput { session });
			}
		}

		Ok(VouchingSessionOutput { session: None })
	}
}

crate::register_core_query!(VouchingSessionQuery, "network.pair.vouching_session");
