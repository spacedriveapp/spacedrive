use super::output::{PairStatusOutput, PairingSessionSummary};
use crate::infra::query::QueryResult;
use crate::service::network::PairingState;
use crate::{context::CoreContext, infra::query::CoreQuery};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PairStatusQueryInput;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PairStatusQuery;

impl CoreQuery for PairStatusQuery {
	type Input = PairStatusQueryInput;
	type Output = PairStatusOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self)
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let mut sessions_out = Vec::new();
		if let Some(net) = context.get_networking().await {
			let sessions = net.get_pairing_status().await.unwrap_or_default();
			for s in sessions.into_iter() {
				// Extract confirmation info from AwaitingUserConfirmation state
				let (confirmation_code, confirmation_expires_at) = match &s.state {
					PairingState::AwaitingUserConfirmation {
						confirmation_code,
						expires_at,
					} => (Some(confirmation_code.clone()), Some(*expires_at)),
					_ => (s.confirmation_code.clone(), s.confirmation_expires_at),
				};

				// Extract device info
				let (remote_device_name, remote_device_os) =
					if let Some(ref info) = s.remote_device_info {
						(Some(info.device_name.clone()), Some(info.os_version.clone()))
					} else {
						(None, None)
					};

				sessions_out.push(PairingSessionSummary {
					id: s.id,
					state: s.state.into(),
					remote_device_id: s.remote_device_id,
					remote_device_name,
					remote_device_os,
					expires_at: None,
					confirmation_code,
					confirmation_expires_at,
				});
			}
		}
		Ok(PairStatusOutput {
			sessions: sessions_out,
		})
	}
}

crate::register_core_query!(PairStatusQuery, "network.pair.status");
