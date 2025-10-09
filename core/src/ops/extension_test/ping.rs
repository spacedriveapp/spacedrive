//! Ping/Pong test operation
//!
//! Simple query that echoes back input to validate WASM integration.

use crate::{
	context::CoreContext,
	infra::{
		api::SessionContext,
		query::{LibraryQuery, QueryResult},
	},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PingInput {
	pub message: String,
	#[serde(default)]
	pub count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PingOutput {
	pub echo: String,
	pub count: u32,
	pub extension_works: bool,
}

/// Ping test query - validates WASM integration
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PingQuery {
	input: PingInput,
}

impl LibraryQuery for PingQuery {
	type Input = PingInput;
	type Output = PingOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		_context: Arc<CoreContext>,
		_session: SessionContext,
	) -> QueryResult<Self::Output> {
		tracing::info!(
			message = %self.input.message,
			count = ?self.input.count,
			"🎉 Ping query called from extension! WASM integration works!"
		);

		Ok(PingOutput {
			echo: format!("Pong: {}", self.input.message),
			count: self.input.count.unwrap_or(1),
			extension_works: true,
		})
	}
}

// Register with Wire system
crate::register_library_query!(PingQuery, "test.ping");

