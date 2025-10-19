//! Shared connection management utilities for Iroh networking
//!
//! Provides connection caching helpers following Iroh best practices:
//! - One persistent connection per device pair
//! - Lightweight streams for individual messages (0 RTT overhead)
//! - Automatic connection reuse across all protocols

use crate::service::network::{NetworkingError, Result};
use iroh::{endpoint::Connection, Endpoint, NodeAddr, NodeId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::logging::NetworkLogger;

/// Get or create a connection to a specific node
///
/// This implements Iroh's best practice of reusing persistent connections
/// and creating new streams for each message exchange.
///
/// # Arguments
/// * `connections` - Shared connection cache (all protocols use the same cache)
/// * `endpoint` - Iroh endpoint for creating new connections
/// * `node_id` - Target node to connect to
/// * `alpn` - Protocol ALPN identifier
/// * `logger` - Logger for connection events
///
/// # Returns
/// * `Ok(Connection)` - Either cached or newly created connection
/// * `Err(NetworkingError)` - If connection fails
pub async fn get_or_create_connection(
	connections: Arc<RwLock<HashMap<NodeId, Connection>>>,
	endpoint: &Endpoint,
	node_id: NodeId,
	alpn: &'static [u8],
	logger: &Arc<dyn NetworkLogger>,
) -> Result<Connection> {
	// Check cache first
	{
		let connections_guard = connections.read().await;
		if let Some(conn) = connections_guard.get(&node_id) {
			if conn.close_reason().is_none() {
				logger
					.debug(&format!("Reusing existing connection to node {}", node_id))
					.await;
				return Ok(conn.clone());
			} else {
				logger
					.debug(&format!(
						"Cached connection to node {} is closed, creating new one",
						node_id
					))
					.await;
			}
		}
	}

	// Create new connection
	let node_addr = NodeAddr::new(node_id);
	logger
		.debug(&format!("Creating new connection to node {}", node_id))
		.await;

	let conn = endpoint
		.connect(node_addr, alpn)
		.await
		.map_err(|e| NetworkingError::ConnectionFailed(format!("Failed to connect: {}", e)))?;

	// Cache the connection
	{
		let mut connections_guard = connections.write().await;
		connections_guard.insert(node_id, conn.clone());
	}

	logger
		.info(&format!("Created new connection to node {}", node_id))
		.await;

	Ok(conn)
}
