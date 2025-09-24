use super::output::{PairStatusOutput, PairingSessionSummary};
use crate::{context::CoreContext, cqrs::CoreQuery};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PairStatusQueryInput;

#[derive(Debug, Clone)]
pub struct PairStatusQuery;

impl CoreQuery for PairStatusQuery {
	type Input = PairStatusQueryInput;
	type Output = PairStatusOutput;

	fn from_input(input: Self::Input) -> Result<Self> {
		Ok(Self)
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> Result<Self::Output> {
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
		Ok(PairStatusOutput {
			sessions: sessions_out,
		})
	}
}

crate::register_core_query!(PairStatusQuery, "network.pair.status");
