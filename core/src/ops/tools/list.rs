use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, QueryResult},
	service::external_tools::ExternalToolStatus,
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ToolListOutput {
	pub tools: Vec<ExternalToolStatus>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct ToolListInput {}

pub struct ToolsListQuery;

impl CoreQuery for ToolsListQuery {
	type Input = ToolListInput;
	type Output = ToolListOutput;

	fn from_input(_: Self::Input) -> QueryResult<Self> {
		Ok(Self)
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		Ok(ToolListOutput {
			tools: context.external_tools.statuses().await,
		})
	}
}

crate::register_core_query!(ToolsListQuery, "tools.list");
