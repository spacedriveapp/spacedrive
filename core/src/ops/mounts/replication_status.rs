//! What replication is doing right now: paused or not, the cap, and every
//! replica fetch in flight with bytes, total and rate.

use crate::{
	infra::query::{CoreQuery, QueryResult},
	service::mounts::replication::{self, ReplicationStatus},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct MountsReplicationStatusInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MountsReplicationStatusQuery;

impl CoreQuery for MountsReplicationStatusQuery {
	type Input = MountsReplicationStatusInput;
	type Output = ReplicationStatus;

	fn from_input(_input: Self::Input) -> QueryResult<Self> {
		Ok(Self)
	}

	async fn execute(
		self,
		_context: Arc<crate::context::CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		Ok(replication::status())
	}
}

crate::register_core_query!(MountsReplicationStatusQuery, "mounts.replication_status");
