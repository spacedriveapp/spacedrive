use std::collections::HashMap;
use std::sync::Arc;

use futures::future::BoxFuture;
use tokio::sync::RwLock;

use crate::Core;
use bincode::config::standard;
use bincode::serde::{decode_from_slice, encode_to_vec};
use serde::{de::DeserializeOwned, Serialize};

/// Unified handler signature for both actions and queries
/// Actions return empty Vec<u8>, queries return serialized output
pub type OperationHandler =
	Arc<dyn Fn(Vec<u8>, Arc<Core>) -> BoxFuture<'static, Result<Vec<u8>, String>> + Send + Sync>;

/// Registry that maps type IDs to handlers. The daemon remains agnostic of concrete types.
pub struct DispatchRegistry {
	handlers: RwLock<HashMap<String, OperationHandler>>,
}

impl DispatchRegistry {
	pub fn new() -> Arc<Self> {
		Arc::new(Self {
			handlers: RwLock::new(HashMap::new()),
		})
	}

	pub async fn register_handler(&self, type_id: impl Into<String>, handler: OperationHandler) {
		self.handlers.write().await.insert(type_id.into(), handler);
	}

	pub async fn dispatch(
		&self,
		type_id: &str,
		payload: Vec<u8>,
		core: Arc<Core>,
	) -> Result<Vec<u8>, String> {
		let map = self.handlers.read().await;
		match map.get(type_id) {
			Some(handler) => (handler)(payload, core).await,
			None => Err(format!("Unknown operation type: {}", type_id)),
		}
	}
}

/// Build a generic action handler that decodes T from payload, executes, and returns empty Ok bytes
pub fn make_action_handler<T>(
	exec: std::sync::Arc<
		dyn Fn(T, std::sync::Arc<Core>) -> BoxFuture<'static, Result<(), String>>
			+ Send
			+ Sync
			+ 'static,
	>,
) -> OperationHandler
where
	T: DeserializeOwned + Send + 'static,
{
	std::sync::Arc::new(move |payload, core| {
		let exec = exec.clone();
		Box::pin(async move {
			let val: T = decode_from_slice(&payload, standard())
				.map_err(|e| format!("deserialize: {}", e))?
				.0;
			(exec)(val, core).await.map(|_| Vec::new())
		})
	})
}

/// Build a generic query handler that decodes Q, executes to O, and encodes O
pub fn make_query_handler<Q, O>(
	exec: std::sync::Arc<
		dyn Fn(Q, std::sync::Arc<Core>) -> BoxFuture<'static, Result<O, String>>
			+ Send
			+ Sync
			+ 'static,
	>,
) -> OperationHandler
where
	Q: DeserializeOwned + Send + 'static,
	O: Serialize + Send + 'static,
{
	std::sync::Arc::new(move |payload, core| {
		let exec = exec.clone();
		Box::pin(async move {
			let val: Q = decode_from_slice(&payload, standard())
				.map_err(|e| format!("deserialize: {}", e))?
				.0;
			let out = (exec)(val, core).await?;
			encode_to_vec(&out, standard()).map_err(|e| e.to_string())
		})
	})
}
