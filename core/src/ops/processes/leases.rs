//! Read the host's port ledger.

use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, QueryError, QueryResult},
};
use sd_supervisor::PortLedger;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProcessLeasesInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProcessLeasesOutput {
	pub ledger: PortLedger,
}

pub struct ProcessLeasesQuery;

impl CoreQuery for ProcessLeasesQuery {
	type Input = ProcessLeasesInput;
	type Output = ProcessLeasesOutput;

	fn from_input(_input: Self::Input) -> QueryResult<Self> {
		Ok(Self)
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let manager = context
			.processes()
			.await
			.ok_or_else(|| QueryError::Internal("process manager not initialized".to_string()))?;
		Ok(ProcessLeasesOutput {
			ledger: manager.supervisor().ledger().ledger().await,
		})
	}
}

crate::register_core_query!(ProcessLeasesQuery, "processes.leases");
