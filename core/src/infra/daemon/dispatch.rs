use std::collections::HashMap;
use std::sync::Arc;

use futures::future::BoxFuture;
use tokio::sync::RwLock;

use crate::Core;
use super::state::SessionState;

/// Signature for a generic action handler: takes raw bytes, returns raw bytes
pub type ActionHandler = Arc<
	dyn Fn(Vec<u8>, Arc<Core>, SessionState) -> BoxFuture<'static, Result<Vec<u8>, String>>
		+ Send
		+ Sync,
>;

/// Signature for a generic query handler (placeholder for future)
pub type QueryHandler = Arc<
	dyn Fn(Vec<u8>, Arc<Core>, SessionState) -> BoxFuture<'static, Result<Vec<u8>, String>>
		+ Send
		+ Sync,
>;

/// Registry that maps type IDs to handlers. The daemon remains agnostic of concrete types.
pub struct DispatchRegistry {
	actions: RwLock<HashMap<String, ActionHandler>>,
	queries: RwLock<HashMap<String, QueryHandler>>,
}

impl DispatchRegistry {
	pub fn new() -> Arc<Self> {
		Arc::new(Self {
			actions: RwLock::new(HashMap::new()),
			queries: RwLock::new(HashMap::new()),
		})
	}

	pub async fn register_action(&self, type_id: impl Into<String>, handler: ActionHandler) {
		self.actions.write().await.insert(type_id.into(), handler);
	}

	pub async fn register_query(&self, type_id: impl Into<String>, handler: QueryHandler) {
		self.queries.write().await.insert(type_id.into(), handler);
	}

	pub async fn dispatch_action(
		&self,
		type_id: &str,
		payload: Vec<u8>,
		core: Arc<Core>,
		session: SessionState,
	) -> Result<Vec<u8>, String> {
		let map = self.actions.read().await;
		match map.get(type_id) {
			Some(handler) => (handler)(payload, core, session).await,
			None => Err("Unknown action type".into()),
		}
	}

	pub async fn dispatch_query(
		&self,
		type_id: &str,
		payload: Vec<u8>,
		core: Arc<Core>,
		session: SessionState,
	) -> Result<Vec<u8>, String> {
		let map = self.queries.read().await;
		match map.get(type_id) {
			Some(handler) => (handler)(payload, core, session).await,
			None => Err("Unknown query type".into()),
		}
	}
}


