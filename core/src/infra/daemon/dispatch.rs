use std::collections::HashMap;
use std::sync::Arc;

use futures::future::BoxFuture;
use tokio::sync::RwLock;

use super::state::SessionState;
use crate::Core;
use bincode::config::standard;
use bincode::serde::{decode_from_slice, encode_to_vec};
use serde::{de::DeserializeOwned, Serialize};

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

/// Build a generic action handler that decodes T from payload, executes, and returns empty Ok bytes
pub fn make_action_handler<T>(
	exec: std::sync::Arc<
		dyn Fn(T, std::sync::Arc<Core>, SessionState) -> BoxFuture<'static, Result<(), String>>
			+ Send
			+ Sync
			+ 'static,
	>,
) -> ActionHandler
where
	T: DeserializeOwned + Send + 'static,
{
	std::sync::Arc::new(move |payload, core, session| {
		let exec = exec.clone();
		Box::pin(async move {
			let val: T = decode_from_slice(&payload, standard())
				.map_err(|e| format!("deserialize: {}", e))?
				.0;
			(exec)(val, core, session).await.map(|_| Vec::new())
		})
	})
}

/// Build a generic query handler that decodes Q, executes to O, and encodes O
pub fn make_query_handler<Q, O>(
	exec: std::sync::Arc<
		dyn Fn(Q, std::sync::Arc<Core>, SessionState) -> BoxFuture<'static, Result<O, String>>
			+ Send
			+ Sync
			+ 'static,
	>,
) -> QueryHandler
where
	Q: DeserializeOwned + Send + 'static,
	O: Serialize + Send + 'static,
{
	std::sync::Arc::new(move |payload, core, session| {
		let exec = exec.clone();
		Box::pin(async move {
			let val: Q = decode_from_slice(&payload, standard())
				.map_err(|e| format!("deserialize: {}", e))?
				.0;
			let out = (exec)(val, core, session).await?;
			encode_to_vec(&out, standard()).map_err(|e| e.to_string())
		})
	})
}
