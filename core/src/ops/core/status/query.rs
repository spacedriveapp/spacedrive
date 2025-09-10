//! Core status query (modular)

use super::output::CoreStatus;
use crate::{context::CoreContext, cqrs::Query, register_query};
use anyhow::Result;
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CoreStatusQuery;

impl Query for CoreStatusQuery {
	type Output = CoreStatus;

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
		let libs = context.library_manager.list().await;
		Ok(CoreStatus {
			version: env!("CARGO_PKG_VERSION").to_string(),
			library_count: libs.len(),
		})
	}
}

impl crate::client::Wire for CoreStatusQuery {
	const METHOD: &'static str = "query:core.status.v1";
}

register_query!(CoreStatusQuery);
