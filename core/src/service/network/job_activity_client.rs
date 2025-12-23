//! Client for subscribing to job activity from remote devices

use crate::service::network::core::JOB_ACTIVITY_ALPN;
use crate::service::network::{
	device::DeviceRegistry,
	protocol::job_activity::{JobActivityMessage, RemoteJobEvent},
	remote_job_cache::RemoteJobCache,
	utils::{get_or_create_connection, SilentLogger},
	NetworkingError, Result,
};
use iroh::{endpoint::Connection, Endpoint, NodeId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;
use tracing::{error, info};
use uuid::Uuid;

/// Client for subscribing to job activity from remote devices
pub struct JobActivityClient {
	endpoint: Endpoint,
	connections: Arc<RwLock<HashMap<(NodeId, Vec<u8>), Connection>>>,
	remote_cache: Arc<RemoteJobCache>,
	device_registry: Arc<RwLock<DeviceRegistry>>,
}

impl JobActivityClient {
	pub fn new(
		endpoint: Endpoint,
		connections: Arc<RwLock<HashMap<(NodeId, Vec<u8>), Connection>>>,
		remote_cache: Arc<RemoteJobCache>,
		device_registry: Arc<RwLock<DeviceRegistry>>,
	) -> Self {
		Self {
			endpoint,
			connections,
			remote_cache,
			device_registry,
		}
	}

	/// Subscribe to job activity from a remote device
	pub async fn subscribe_to_device(
		&self,
		device_id: Uuid,
		library_id: Option<Uuid>,
	) -> Result<()> {
		// Get node_id from device registry
		let node_id = {
			let registry = self.device_registry.read().await;
			registry
				.get_node_by_device(device_id)
				.ok_or_else(|| NetworkingError::DeviceNotFound(device_id))?
		};

		// Get or create connection
		let logger: Arc<dyn crate::service::network::NetworkLogger> = Arc::new(SilentLogger);
		let conn = get_or_create_connection(
			self.connections.clone(),
			&self.endpoint,
			node_id,
			JOB_ACTIVITY_ALPN,
			&logger,
		)
		.await?;

		// Open stream
		let (mut send, recv) = conn
			.open_bi()
			.await
			.map_err(|e| NetworkingError::ConnectionFailed(format!("open stream: {}", e)))?;

		// Send subscribe message
		let subscribe_msg = JobActivityMessage::Subscribe { library_id };
		let msg_data = rmp_serde::to_vec(&subscribe_msg)
			.map_err(|e| NetworkingError::Protocol(format!("Serialization error: {}", e)))?;

		let len = (msg_data.len() as u32).to_be_bytes();
		send.write_all(&len)
			.await
			.map_err(|e| NetworkingError::Transport(format!("{}", e)))?;
		send.write_all(&msg_data)
			.await
			.map_err(|e| NetworkingError::Transport(format!("{}", e)))?;
		send.flush()
			.await
			.map_err(|e| NetworkingError::Transport(format!("{}", e)))?;

		info!("Subscribed to job activity from device {}", device_id);

		// Spawn receiver task
		let remote_cache = self.remote_cache.clone();
		let device_registry = self.device_registry.clone();

		tokio::spawn(async move {
			Self::receive_events(device_id, recv, remote_cache, device_registry).await;
		});

		Ok(())
	}

	/// Background task to receive and cache events from a remote device
	async fn receive_events(
		device_id: Uuid,
		mut recv: iroh::endpoint::RecvStream,
		remote_cache: Arc<RemoteJobCache>,
		device_registry: Arc<RwLock<DeviceRegistry>>,
	) {
		// Get device name
		let device_name = {
			let registry = device_registry.read().await;
			registry
				.get_device_state(device_id)
				.and_then(|state| match state {
					crate::service::network::device::DeviceState::Paired { info, .. }
					| crate::service::network::device::DeviceState::Connected { info, .. }
					| crate::service::network::device::DeviceState::Disconnected { info, .. } => {
						Some(info.device_name.clone())
					}
					_ => None,
				})
				.unwrap_or_else(|| format!("Device {}", device_id))
		};

		loop {
			// Read length
			let mut len_buf = [0u8; 4];
			if recv.read_exact(&mut len_buf).await.is_err() {
				info!("Job activity stream closed for device {}", device_id);
				break;
			}
			let msg_len = u32::from_be_bytes(len_buf) as usize;

			// Read message
			let mut msg_buf = vec![0u8; msg_len];
			if recv.read_exact(&mut msg_buf).await.is_err() {
				error!("Failed to read from device {}", device_id);
				break;
			}

			// Deserialize
			let message: JobActivityMessage = match rmp_serde::from_slice(&msg_buf) {
				Ok(m) => m,
				Err(e) => {
					error!("Failed to deserialize: {}", e);
					continue;
				}
			};

			// Handle event
			if let JobActivityMessage::JobEvent {
				library_id, event, ..
			} = message
			{
				remote_cache
					.handle_event(device_id, device_name.clone(), library_id, event)
					.await;
			}
		}

		// Clean up cache when stream closes
		remote_cache.remove_device_jobs(device_id).await;
	}
}
