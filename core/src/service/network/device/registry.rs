//! Device registry for centralized state management

use super::{
	ConnectionInfo, DeviceInfo, DevicePersistence, DeviceState, PersistedPairedDevice, SessionKeys,
	TrustLevel,
};
use crate::crypto::key_manager::KeyManager;
use crate::device::DeviceManager;
use crate::infra::event::EventBus;
use crate::service::network::{utils::logging::NetworkLogger, NetworkingError, Result};
use chrono::{DateTime, Utc};
use iroh::{NodeAddr, NodeId};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Central registry for all device state and connections
pub struct DeviceRegistry {
	/// Reference to the device manager for local device info
	device_manager: Arc<DeviceManager>,

	/// Map of device ID to current state
	devices: HashMap<Uuid, DeviceState>,

	/// Map of node ID to device ID for quick lookup
	node_to_device: HashMap<NodeId, Uuid>,

	/// Map of session ID to device ID for pairing lookup
	session_to_device: HashMap<Uuid, Uuid>,

	/// Persistence manager for paired devices
	persistence: DevicePersistence,

	/// Logger for device registry operations
	logger: Arc<dyn NetworkLogger>,

	/// Event bus for emitting resource change events
	event_bus: Option<Arc<EventBus>>,
}

impl DeviceRegistry {
	/// Create a new device registry
	pub fn new(
		device_manager: Arc<DeviceManager>,
		key_manager: Arc<KeyManager>,
		logger: Arc<dyn NetworkLogger>,
	) -> Self {
		let persistence = DevicePersistence::new(key_manager);

		Self {
			device_manager,
			devices: HashMap::new(),
			node_to_device: HashMap::new(),
			session_to_device: HashMap::new(),
			persistence,
			logger,
			event_bus: None,
		}
	}

	/// Set the event bus for emitting resource change events
	pub fn set_event_bus(&mut self, event_bus: Arc<EventBus>) {
		self.event_bus = Some(event_bus);
	}

	/// Emit a ResourceChanged event for a device
	fn emit_device_changed(&self, device_id: Uuid, info: &DeviceInfo, is_connected: bool) {
		let Some(event_bus) = &self.event_bus else {
			return;
		};

		// Convert network DeviceInfo to domain Device
		let device = crate::domain::Device::from_network_info(info, is_connected);

		use crate::domain::resource::EventEmitter;
		if let Err(e) = device.emit_changed(event_bus) {
			tracing::warn!(
				device_id = %device_id,
				error = %e,
				"Failed to emit device ResourceChanged event"
			);
		}
	}

	/// Load paired devices from persistence on startup
	pub async fn load_paired_devices(&mut self) -> Result<Vec<Uuid>> {
		let paired_devices = self.persistence.load_paired_devices().await?;
		let mut loaded_device_ids = Vec::new();

		for (device_id, persisted_device) in paired_devices {
			// Add device to registry in Paired state
			let state = DeviceState::Paired {
				info: persisted_device.device_info.clone(),
				session_keys: persisted_device.session_keys.clone(),
				paired_at: persisted_device.paired_at,
			};

			self.devices.insert(device_id, state);
			loaded_device_ids.push(device_id);

			// Restore node-to-device mapping so incoming connections can find this device
			if let Ok(node_id) = persisted_device
				.device_info
				.network_fingerprint
				.node_id
				.parse::<NodeId>()
			{
				self.node_to_device.insert(node_id, device_id);
				self.logger
					.debug(&format!(
						"Restored node-to-device mapping for {}: {} -> {}",
						persisted_device.device_info.device_name, node_id, device_id
					))
					.await;
			} else {
				self.logger
					.warn(&format!(
						"Failed to parse node ID for device {} during load",
						persisted_device.device_info.device_name
					))
					.await;
			}

			// Cache the paired device slug for pre-library address resolution
			if let Err(e) = self
				.device_manager
				.cache_paired_device(persisted_device.device_info.device_slug.clone(), device_id)
			{
				self.logger
					.warn(&format!(
						"Failed to cache paired device slug for {}: {}",
						persisted_device.device_info.device_name, e
					))
					.await;
			}
		}

		Ok(loaded_device_ids)
	}

	/// Get devices that should auto-reconnect
	pub async fn get_auto_reconnect_devices(&self) -> Result<Vec<(Uuid, PersistedPairedDevice)>> {
		self.persistence.get_auto_reconnect_devices().await
	}

	/// Add a discovered node
	pub fn add_discovered_node(&mut self, device_id: Uuid, node_id: NodeId, node_addr: NodeAddr) {
		let state = DeviceState::Discovered {
			node_id,
			node_addr,
			discovered_at: Utc::now(),
		};

		self.devices.insert(device_id, state);
		self.node_to_device.insert(node_id, device_id);
	}

	/// Start pairing process for a device
	pub fn start_pairing(
		&mut self,
		device_id: Uuid,
		node_id: NodeId,
		session_id: Uuid,
		node_addr: NodeAddr,
	) -> Result<()> {
		let state = DeviceState::Pairing {
			node_id,
			session_id,
			node_addr,
			started_at: Utc::now(),
		};

		self.devices.insert(device_id, state);
		self.node_to_device.insert(node_id, device_id);
		self.session_to_device.insert(session_id, device_id);

		Ok(())
	}

	pub fn get_device_name(&self) -> Result<String> {
		let config = self.device_manager.config().map_err(|e| {
			NetworkingError::Protocol(format!("Failed to get device config: {}", e))
		})?;
		Ok(config.name)
	}

	/// Complete pairing for a device
	pub async fn complete_pairing(
		&mut self,
		device_id: Uuid,
		info: DeviceInfo,
		session_keys: SessionKeys,
		relay_url: Option<String>,
	) -> Result<()> {
		// Parse node ID from network fingerprint
		let node_id = info
			.network_fingerprint
			.node_id
			.parse::<NodeId>()
			.map_err(|e| {
				NetworkingError::Protocol(format!("Invalid node ID in network fingerprint: {}", e))
			})?;

		// Add node-to-device mapping so device can be found for messaging
		self.node_to_device.insert(node_id, device_id);
		self.logger
			.debug(&format!(
				"Added node-to-device mapping: {} -> {}",
				node_id, device_id
			))
			.await;

		let state = DeviceState::Paired {
			info: info.clone(),
			session_keys: session_keys.clone(),
			paired_at: Utc::now(),
		};

		self.devices.insert(device_id, state);

		// Cache the paired device slug for pre-library address resolution
		if let Err(e) = self
			.device_manager
			.cache_paired_device(info.device_slug.clone(), device_id)
		{
			self.logger
				.warn(&format!(
					"Failed to cache paired device slug for {}: {}",
					info.device_name, e
				))
				.await;
		} else {
			self.logger
				.debug(&format!(
					"Cached device slug: {} -> {}",
					info.device_slug, device_id
				))
				.await;
		}

		// Persist the paired device for future reconnection (with relay_url for optimization)
		if let Err(e) = self
			.persistence
			.add_paired_device(device_id, info.clone(), session_keys.clone(), relay_url)
			.await
		{
			self.logger
				.warn(&format!(
					"Failed to persist paired device {}: {}",
					device_id, e
				))
				.await;
			// Continue anyway - pairing succeeded even if persistence failed
		} else {
			self.logger
				.debug(&format!("Persisted paired device: {}", device_id))
				.await;
		}

		self.logger
			.info(&format!(
				"Paired device {} (slug: {}, id: {})",
				info.device_name, info.device_slug, device_id
			))
			.await;

		// Emit ResourceChanged event for UI reactivity
		self.emit_device_changed(device_id, &info, false);

		Ok(())
	}

	/// Mark device as connected
	pub async fn mark_connected(
		&mut self,
		device_id: Uuid,
		connection: ConnectionInfo,
	) -> Result<()> {
		let current_state = self
			.devices
			.get(&device_id)
			.ok_or_else(|| NetworkingError::DeviceNotFound(device_id))?;

		let (info, session_keys): (DeviceInfo, super::SessionKeys) = match current_state {
			DeviceState::Paired {
				info, session_keys, ..
			} => (info.clone(), session_keys.clone()),
			DeviceState::Disconnected {
				info, session_keys, ..
			} => (info.clone(), session_keys.clone()),
			DeviceState::Discovered { node_id, .. } => {
				// Need device info - this shouldn't happen normally
				return Err(NetworkingError::Protocol(
					"Cannot connect to unpaired device".to_string(),
				));
			}
			DeviceState::Connected { .. } => {
				return Err(NetworkingError::Protocol(
					"Device already connected".to_string(),
				));
			}
			DeviceState::Pairing { .. } => {
				return Err(NetworkingError::Protocol(
					"Device still pairing".to_string(),
				));
			}
		};

		let state = DeviceState::Connected {
			info: info.clone(),
			connection,
			session_keys,
			connected_at: Utc::now(),
		};

		self.devices.insert(device_id, state);

		// Update persistence - device connected successfully
		if let Err(e) = self
			.persistence
			.update_device_connection(device_id, true, None)
			.await
		{
			self.logger
				.warn(&format!(
					"Failed to update device connection status {}: {}",
					device_id, e
				))
				.await;
		}

		// Emit ResourceChanged event for UI reactivity
		self.emit_device_changed(device_id, &info, true);

		Ok(())
	}

	/// Mark device as disconnected
	pub async fn mark_disconnected(
		&mut self,
		device_id: Uuid,
		reason: super::DisconnectionReason,
	) -> Result<()> {
		let current_state = self
			.devices
			.get(&device_id)
			.ok_or_else(|| NetworkingError::DeviceNotFound(device_id))?;

		let (info, session_keys) = match current_state {
			DeviceState::Connected {
				info, session_keys, ..
			} => (info.clone(), session_keys.clone()),
			DeviceState::Paired {
				info, session_keys, ..
			} => (info.clone(), session_keys.clone()),
			_ => {
				return Err(NetworkingError::Protocol(
					"Cannot disconnect device that isn't connected".to_string(),
				));
			}
		};

		let state = DeviceState::Disconnected {
			info: info.clone(),
			session_keys,
			last_seen: Utc::now(),
			reason,
		};

		self.devices.insert(device_id, state);

		// Update persistence - device disconnected
		if let Err(e) = self
			.persistence
			.update_device_connection(device_id, false, None)
			.await
		{
			self.logger
				.warn(&format!(
					"Failed to update device disconnection status {}: {}",
					device_id, e
				))
				.await;
		}

		// Emit ResourceChanged event for UI reactivity
		self.emit_device_changed(device_id, &info, false);

		Ok(())
	}

	/// Get device state by device ID
	pub fn get_device_state(&self, device_id: Uuid) -> Option<&DeviceState> {
		self.devices.get(&device_id)
	}

	/// Get device ID by peer ID
	pub fn get_device_by_node(&self, node_id: NodeId) -> Option<Uuid> {
		self.node_to_device.get(&node_id).copied()
	}

	/// Get device ID by session ID
	pub fn get_device_by_session(&self, session_id: Uuid) -> Option<Uuid> {
		self.session_to_device.get(&session_id).copied()
	}

	/// Get all devices with their IDs and states
	pub fn get_all_devices(&self) -> Vec<(Uuid, DeviceState)> {
		self.devices
			.iter()
			.map(|(id, state)| (*id, state.clone()))
			.collect()
	}

	/// Get all connected devices
	pub fn get_connected_devices(&self) -> Vec<DeviceInfo> {
		self.devices
			.values()
			.filter_map(|state| match state {
				DeviceState::Connected { info, .. } => Some(info.clone()),
				_ => None,
			})
			.collect()
	}

	/// Get all paired devices (including disconnected)
	pub fn get_paired_devices(&self) -> Vec<DeviceInfo> {
		self.devices
			.values()
			.filter_map(|state| match state {
				DeviceState::Paired { info, .. } => Some(info.clone()),
				DeviceState::Connected { info, .. } => Some(info.clone()),
				DeviceState::Disconnected { info, .. } => Some(info.clone()),
				_ => None,
			})
			.collect()
	}

	/// Remove a device from the registry
	pub fn remove_device(&mut self, device_id: Uuid) -> Result<()> {
		if let Some(state) = self.devices.remove(&device_id) {
			// Clean up node-to-device mappings for all states
			match &state {
				DeviceState::Discovered { node_id, .. } | DeviceState::Pairing { node_id, .. } => {
					self.node_to_device.remove(node_id);
				}
				DeviceState::Paired { info, .. }
				| DeviceState::Connected { info, .. }
				| DeviceState::Disconnected { info, .. } => {
					// Extract node ID from network fingerprint and clean up mapping
					if let Ok(node_id) = info.network_fingerprint.node_id.parse::<iroh::NodeId>() {
						self.node_to_device.remove(&node_id);
					}
				}
			}

			// Clean up session-to-device mapping for pairing state
			if let DeviceState::Pairing { session_id, .. } = &state {
				self.session_to_device.remove(session_id);
			}
		}

		Ok(())
	}

	/// Remove a paired device from persistence
	pub async fn remove_paired_device(&self, device_id: Uuid) -> Result<bool> {
		self.persistence.remove_paired_device(device_id).await
	}

	/// Get peer ID for a device
	pub fn get_node_by_device(&self, device_id: Uuid) -> Option<NodeId> {
		// Look through node_to_device map in reverse
		for (node_id, &dev_id) in &self.node_to_device {
			if dev_id == device_id {
				// Found node for device
				return Some(*node_id);
			}
		}
		// No peer found for device - check node_to_device mappings
		None
	}

	/// Get node ID for a device (alias for get_node_by_device)
	pub fn get_node_id_for_device(&self, device_id: Uuid) -> Option<NodeId> {
		self.get_node_by_device(device_id)
	}

	/// Check if a device is currently connected according to Iroh
	///
	/// This is the canonical way to check device connectivity. It queries Iroh's endpoint
	/// directly to get real-time connection state, rather than relying on cached state.
	///
	/// Returns true if:
	/// - Device UUID is mapped to a NodeId
	/// - Iroh reports an active connection (Direct, Relay, or Mixed)
	/// - Connection type is not None
	pub fn is_node_connected(&self, endpoint: &iroh::Endpoint, device_id: Uuid) -> bool {
		// Get NodeId for this device
		let node_id = match self.get_node_id_for_device(device_id) {
			Some(id) => id,
			None => return false,
		};

		// Query Iroh for current connection state
		match endpoint.remote_info(node_id) {
			Some(remote_info) => {
				// Check if connection type indicates an active connection
				!matches!(remote_info.conn_type, iroh::endpoint::ConnectionType::None)
			}
			None => false,
		}
	}

	/// Get device UUID from node ID
	pub fn get_device_by_node_id(&self, node_id: NodeId) -> Option<Uuid> {
		self.node_to_device.get(&node_id).copied()
	}

	/// Update device connection state from Iroh RemoteInfo
	///
	/// This is called by the connection monitor to update DeviceRegistry state
	/// based on Iroh's actual connection state. This is cosmetic only - sync
	/// routing uses is_node_connected() which queries Iroh directly.
	pub async fn update_device_from_connection(
		&mut self,
		device_id: Uuid,
		node_id: NodeId,
		conn_type: iroh::endpoint::ConnectionType,
		latency: Option<std::time::Duration>,
	) -> Result<()> {
		// Update node-to-device mapping
		self.node_to_device.insert(node_id, device_id);

		// Get current device state
		let current_state = match self.devices.get(&device_id) {
			Some(state) => state.clone(),
			None => return Ok(()), // Device not in registry
		};

		// Determine if we should be in Connected state
		let should_be_connected = !matches!(conn_type, iroh::endpoint::ConnectionType::None);

		match current_state {
			DeviceState::Paired {
				info, session_keys, ..
			} if should_be_connected => {
				// Transition from Paired to Connected
				let state = DeviceState::Connected {
					info: info.clone(),
					session_keys,
					connected_at: Utc::now(),
					connection: ConnectionInfo {
						latency_ms: latency.map(|d| d.as_millis() as u32),
						rx_bytes: 0,
						tx_bytes: 0,
					},
				};
				self.devices.insert(device_id, state);

				// Update persistence
				self.persistence
					.update_device_connection(device_id, true, None)
					.await
					.ok();

				// Emit ResourceChanged event for UI reactivity
				self.emit_device_changed(device_id, &info, true);
			}
			DeviceState::Connected {
				info,
				session_keys,
				connected_at,
				mut connection,
			} if should_be_connected => {
				// Already connected, just update latency
				connection.latency_ms = latency.map(|d| d.as_millis() as u32);
				let state = DeviceState::Connected {
					info,
					session_keys,
					connected_at,
					connection,
				};
				self.devices.insert(device_id, state);
				// No event emission for latency-only updates
			}
			DeviceState::Connected {
				info, session_keys, ..
			} if !should_be_connected => {
				// Transition from Connected to Paired (connection lost)
				let state = DeviceState::Paired {
					info: info.clone(),
					session_keys,
					paired_at: Utc::now(),
				};
				self.devices.insert(device_id, state);

				// Update persistence
				self.persistence
					.update_device_connection(device_id, false, None)
					.await
					.ok();

				// Emit ResourceChanged event for UI reactivity
				self.emit_device_changed(device_id, &info, false);
			}
			_ => {
				// No state change needed
			}
		}

		Ok(())
	}

	/// Get session keys for a device
	pub fn get_session_keys(&self, device_id: Uuid) -> Option<super::SessionKeys> {
		match self.devices.get(&device_id) {
			Some(DeviceState::Paired { session_keys, .. }) => {
				// Found session keys for paired device
				Some(session_keys.clone())
			}
			Some(DeviceState::Connected { session_keys, .. }) => {
				// Found session keys for connected device
				Some(session_keys.clone())
			}
			Some(DeviceState::Disconnected { session_keys, .. }) => {
				// Found session keys for disconnected device (session keys are preserved after disconnect)
				Some(session_keys.clone())
			}
			_ => {
				// Device not found or in a state without session keys (Discovered, Pairing)
				None
			}
		}
	}

	/// Get all currently connected peer IDs
	pub fn get_connected_nodes(&self) -> Vec<NodeId> {
		self.node_to_device.keys().cloned().collect()
	}

	/// Get our local device info
	pub fn get_local_device_info(&self) -> Result<DeviceInfo> {
		let device_id = self
			.device_manager
			.device_id()
			.map_err(|e| NetworkingError::Protocol(format!("Failed to get device ID: {}", e)))?;

		let config = self.device_manager.config().map_err(|e| {
			NetworkingError::Protocol(format!("Failed to get device config: {}", e))
		})?;
		let device_name = config.name;
		let device_slug = config.slug;

		// TODO: Get actual values from device manager or system
		Ok(DeviceInfo {
			device_id,
			device_name,
			device_slug,
			device_type: super::DeviceType::Desktop, // TODO: Detect actual device type
			os_version: std::env::consts::OS.to_string(),
			app_version: env!("CARGO_PKG_VERSION").to_string(),
			network_fingerprint: crate::service::network::utils::identity::NetworkFingerprint {
				node_id: "placeholder".to_string(), // Will be filled in by caller
				public_key_hash: "placeholder".to_string(),
			},
			last_seen: Utc::now(),
		})
	}

	/// Clean up expired sessions and old disconnected devices
	pub fn cleanup_expired(&mut self) {
		let now = Utc::now();
		let mut to_remove = Vec::new();
		let mut session_mappings_to_remove = Vec::new();

		for (device_id, state) in &self.devices {
			match state {
				DeviceState::Pairing {
					started_at,
					session_id,
					..
				} => {
					// Remove pairing sessions older than 10 minutes
					if now.signed_duration_since(*started_at).num_minutes() > 10 {
						to_remove.push(*device_id);
						session_mappings_to_remove.push(*session_id);
					}
				}
				DeviceState::Paired { paired_at, .. } => {
					// Remove session mappings for paired devices older than 1 hour
					// (pairing completed successfully, no need to keep session mapping)
					if now.signed_duration_since(*paired_at).num_hours() > 1 {
						// Find session ID to remove
						for (session_id, &dev_id) in &self.session_to_device {
							if dev_id == *device_id {
								session_mappings_to_remove.push(*session_id);
							}
						}
					}
				}
				DeviceState::Connected { .. } => {
					// Remove session mappings for connected devices
					// (no longer needed for active connections)
					for (session_id, &dev_id) in &self.session_to_device {
						if dev_id == *device_id {
							session_mappings_to_remove.push(*session_id);
						}
					}
				}
				DeviceState::Disconnected { last_seen, .. } => {
					// Remove disconnected devices older than 7 days
					if now.signed_duration_since(*last_seen).num_days() > 7 {
						to_remove.push(*device_id);
					}
				}
				_ => {}
			}
		}

		// Remove expired devices
		for device_id in to_remove {
			let _ = self.remove_device(device_id);
		}

		// Remove expired session mappings
		for session_id in session_mappings_to_remove {
			self.session_to_device.remove(&session_id);
		}
	}

	/// Set a device as connected with its node ID
	pub async fn set_device_connected(&mut self, device_id: Uuid, node_id: NodeId) -> Result<()> {
		self.logger
			.info(&format!("Setting device {} as connected", device_id))
			.await;

		// Update the node_to_device mapping
		self.node_to_device.insert(node_id, device_id);

		// Extract info from current state for event emission
		let info_for_event: Option<DeviceInfo> = {
			let current_state = self
				.devices
				.get(&device_id)
				.ok_or_else(|| NetworkingError::DeviceNotFound(device_id))?;

			match current_state {
				DeviceState::Paired {
					info, session_keys, ..
				} => {
					let info_clone = info.clone();
					let state = DeviceState::Connected {
						info: info.clone(),
						session_keys: session_keys.clone(),
						connected_at: Utc::now(),
						connection: ConnectionInfo {
							latency_ms: None,
							rx_bytes: 0,
							tx_bytes: 0,
						},
					};
					self.devices.insert(device_id, state);

					if let Err(e) = self
						.persistence
						.update_device_connection(device_id, true, None)
						.await
					{
						self.logger
							.warn(&format!("Failed to update device connection info: {}", e))
							.await;
					}

					Some(info_clone)
				}
				DeviceState::Connected { .. } => {
					self.logger
						.debug(&format!(
							"Device {} already connected, updating node mapping",
							device_id
						))
						.await;
					None // No state change
				}
				DeviceState::Disconnected {
					info, session_keys, ..
				} => {
					let info_clone = info.clone();
					let state = DeviceState::Connected {
						info: info.clone(),
						session_keys: session_keys.clone(),
						connected_at: Utc::now(),
						connection: ConnectionInfo {
							latency_ms: None,
							rx_bytes: 0,
							tx_bytes: 0,
						},
					};
					self.devices.insert(device_id, state);

					if let Err(e) = self
						.persistence
						.update_device_connection(device_id, true, None)
						.await
					{
						self.logger
							.warn(&format!("Failed to update device connection info: {}", e))
							.await;
					}

					Some(info_clone)
				}
				_ => {
					return Err(NetworkingError::Protocol(
						"Device must be paired before connecting".to_string(),
					));
				}
			}
		};

		// Emit ResourceChanged event for UI reactivity (after releasing borrow)
		if let Some(info) = info_for_event {
			self.emit_device_changed(device_id, &info, true);
		}

		Ok(())
	}
}
