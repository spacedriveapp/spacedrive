//! Shared connection management utilities for Iroh networking
//!
//! Provides connection caching helpers following Iroh best practices:
//! - One persistent connection per device pair
//! - Lightweight streams for individual messages (0 RTT overhead)
//! - Automatic connection reuse across all protocols

use crate::service::network::core::event_loop::EventLoopCommand;
use crate::service::network::{NetworkingError, Result};
use iroh::{endpoint::Connection, Endpoint, EndpointAddr, EndpointId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

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
/// * `command_sender` - Event loop handle; a connection this call dials is
///   handed to it so the event loop accepts streams the peer opens on it
/// * `logger` - Logger for connection events
///
/// # Returns
/// * `Ok(Connection)` - Either cached or newly created connection
/// * `Err(NetworkingError)` - If connection fails
pub async fn get_or_create_connection(
	connections: Arc<RwLock<HashMap<(EndpointId, Vec<u8>), Connection>>>,
	endpoint: &Endpoint,
	node_id: EndpointId,
	alpn: &'static [u8],
	command_sender: Option<&mpsc::UnboundedSender<EventLoopCommand>>,
	logger: &Arc<dyn NetworkLogger>,
) -> Result<Connection> {
	let alpn_vec = alpn.to_vec();
	let cache_key = (node_id, alpn_vec.clone());

	// Check cache first (keyed by both node_id AND alpn)
	{
		let connections_guard = connections.read().await;
		if let Some(conn) = connections_guard.get(&cache_key) {
			if conn.close_reason().is_none() {
				logger
					.debug(&format!(
						"Reusing existing {} connection to node {}",
						String::from_utf8_lossy(alpn),
						node_id
					))
					.await;
				return Ok(conn.clone());
			} else {
				logger
					.debug(&format!(
						"Cached {} connection to node {} is closed, creating new one",
						String::from_utf8_lossy(alpn),
						node_id
					))
					.await;
			}
		}
	}

	// Create new connection with specified ALPN
	let node_addr = EndpointAddr::new(node_id);
	logger
		.info(&format!(
			"Creating new {} connection to node {}",
			String::from_utf8_lossy(alpn),
			node_id
		))
		.await;

	let conn = endpoint
		.connect(node_addr, alpn)
		.await
		.map_err(|e| NetworkingError::ConnectionFailed(format!("Failed to connect: {}", e)))?;

	// Cache the connection with (node_id, alpn) key
	{
		let mut connections_guard = connections.write().await;
		connections_guard.insert(cache_key, conn.clone());
	}

	// The cache is shared with the accept side, so the peer finds this
	// connection too and opens its own streams on it. Nobody reads those
	// streams unless the event loop serves the connection like an inbound
	// one; without this the peer's request times out while ours succeed.
	if let Some(sender) = command_sender {
		let _ = sender.send(EventLoopCommand::TrackOutboundConnection {
			node_id,
			conn: conn.clone(),
		});
	}

	logger
		.info(&format!(
			"Created {} connection to node {}",
			String::from_utf8_lossy(alpn),
			node_id
		))
		.await;

	Ok(conn)
}
