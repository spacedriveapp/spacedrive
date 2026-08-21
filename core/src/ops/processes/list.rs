//! List every supervised service on this machine.

use super::resource::Process;
use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, QueryError, QueryResult},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProcessListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProcessListOutput {
	pub processes: Vec<Process>,
}

pub struct ProcessListQuery;

impl CoreQuery for ProcessListQuery {
	type Input = ProcessListInput;
	type Output = ProcessListOutput;

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
		let processes = manager
			.supervisor()
			.status()
			.await
			.into_iter()
			.map(Process::from)
			.collect();
		Ok(ProcessListOutput { processes })
	}
}

crate::register_core_query!(ProcessListQuery, "processes.list");
