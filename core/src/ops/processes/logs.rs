//! Tail one supervised service's log.

use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, QueryError, QueryResult},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

const DEFAULT_LINES: u32 = 200;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProcessLogsInput {
	pub name: String,
	/// How many lines from the end of the log; defaults to 200.
	#[serde(default)]
	pub lines: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProcessLogsOutput {
	pub lines: Vec<String>,
}

pub struct ProcessLogsQuery {
	name: String,
	lines: u32,
}

impl CoreQuery for ProcessLogsQuery {
	type Input = ProcessLogsInput;
	type Output = ProcessLogsOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self {
			name: input.name,
			lines: input.lines.unwrap_or(DEFAULT_LINES),
		})
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
		let lines = manager
			.supervisor()
			.logs(&self.name, self.lines as usize)
			.await
			.map_err(|err| QueryError::Internal(err.to_string()))?;
		Ok(ProcessLogsOutput { lines })
	}
}

crate::register_core_query!(ProcessLogsQuery, "processes.logs");
