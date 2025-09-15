use super::output::{PairStatusOutput, PairingSessionSummary};
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
				sessions_out.push(PairingSessionSummary {
					id: s.id,
					state: s.state.into(),
					remote_device_id: s.remote_device_id,
					expires_at: None,
				});
			}
		}
		Ok(PairStatusOutput { sessions: sessions_out })
	}
}

crate::register_query!(PairStatusQuery, "network.pair.status");

