//! Core status query (modular)

use super::output::CoreStatus;
use crate::{context::CoreContext, cqrs::Query};
use anyhow::Result;
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CoreStatusQuery;

impl Query for CoreStatusQuery {
	type Output = CoreStatus;

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
		let libs = context.libraries().await.list().await;
		Ok(CoreStatus {
			version: env!("CARGO_PKG_VERSION").to_string(),
			library_count: libs.len(),
		})
	}
}

crate::register_query!(CoreStatusQuery, "core.status");
