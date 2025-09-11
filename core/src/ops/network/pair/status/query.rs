use super::output::{PairStatusOutput, PairingSessionSummary, PairingStateLite};
use crate::{context::CoreContext, cqrs::Query};
use anyhow::Result;
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PairStatusQuery;

impl Query for PairStatusQuery {
	type Output = PairStatusOutput;

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
		let mut sessions_out = Vec::new();
		if let Some(net) = context.get_networking().await {
			let sessions = net.get_pairing_status().await.unwrap_or_default();
			for s in sessions.into_iter() {
				let state = match s.state {
					crate::service::network::PairingState::Idle => PairingStateLite::Idle,
					crate::service::network::PairingState::GeneratingCode => PairingStateLite::GeneratingCode,
					crate::service::network::PairingState::Broadcasting => PairingStateLite::Broadcasting,
					crate::service::network::PairingState::Scanning => PairingStateLite::Scanning,
					crate::service::network::PairingState::WaitingForConnection => PairingStateLite::WaitingForConnection,
					crate::service::network::PairingState::Connecting => PairingStateLite::Connecting,
					crate::service::network::PairingState::Authenticating => PairingStateLite::Authenticating,
					crate::service::network::PairingState::ExchangingKeys => PairingStateLite::ExchangingKeys,
					crate::service::network::PairingState::AwaitingConfirmation => PairingStateLite::AwaitingConfirmation,
					crate::service::network::PairingState::EstablishingSession => PairingStateLite::EstablishingSession,
					crate::service::network::PairingState::ChallengeReceived { .. } => PairingStateLite::Challenge,
					crate::service::network::PairingState::ResponsePending { .. } => PairingStateLite::ResponsePending,
					crate::service::network::PairingState::ResponseSent => PairingStateLite::ResponseSent,
					crate::service::network::PairingState::Completed => PairingStateLite::Completed,
					crate::service::network::PairingState::Failed { reason } => PairingStateLite::Failed { reason },
				};
				sessions_out.push(PairingSessionSummary {
					id: s.id,
					state,
					remote_device_id: s.remote_device_id,
					expires_at: None,
				});
			}
		}
		Ok(PairStatusOutput { sessions: sessions_out })
	}
}

crate::register_query!(PairStatusQuery, "network.pair.status");

