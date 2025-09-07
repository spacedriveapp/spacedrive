//! Device registry for centralized state management

use super::{DeviceConnection, DeviceInfo, DeviceState, DevicePersistence, PersistedPairedDevice, SessionKeys, TrustLevel};
use crate::device::DeviceManager;
use crate::service::network::{NetworkingError, Result, utils::logging::NetworkLogger};
use chrono::{DateTime, Utc};
use iroh::net::NodeAddr;
use iroh::net::key::NodeId;
use std::collections::HashMap;
use std::path::Path;
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
}

impl DeviceRegistry {
	/// Create a new device registry
	pub fn new(device_manager: Arc<DeviceManager>, data_dir: impl AsRef<Path>, logger: Arc<dyn NetworkLogger>) -> Result<Self> {
		let persistence = DevicePersistence::new(data_dir)?;

		Ok(Self {
			device_manager,
			devices: HashMap::new(),
			node_to_device: HashMap::new(),
			session_to_device: HashMap::new(),
			persistence,
			logger,
		})
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
		}

		Ok(loaded_device_ids)
	}

	/// Get devices that should auto-reconnect
	pub async fn get_auto_reconnect_devices(&self) -> Result<Vec<(Uuid, PersistedPairedDevice)>> {
		self.persistence.get_auto_reconnect_devices().await
	}

	/// Add a discovered node
	pub fn add_discovered_node(
		&mut self,
		device_id: Uuid,
		node_id: NodeId,
		node_addr: NodeAddr,
	) {
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
	) -> Result<()> {
		let state = DeviceState::Pairing {
			node_id,
			session_id,
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
	) -> Result<()> {
		// Parse node ID from network fingerprint
		let node_id = info.network_fingerprint.node_id.parse::<NodeId>()
			.map_err(|e| NetworkingError::Protocol(format!("Invalid node ID in network fingerprint: {}", e)))?;

		// Add node-to-device mapping so device can be found for messaging
		self.node_to_device.insert(node_id, device_id);
		self.logger.debug(&format!("Added node-to-device mapping: {} -> {}", node_id, device_id)).await;

		// Get current addresses from any existing state
		let mut addresses: Vec<String> = match self.devices.get(&device_id) {
			Some(DeviceState::Discovered { node_addr, .. }) => {
				// Convert NodeAddr to string addresses
				node_addr.direct_addresses().map(|addr| addr.to_string()).collect()
			}
			Some(DeviceState::Connected { connection, .. }) => {
				// If somehow already connected, use those addresses
				connection.addresses.clone()
			}
			_ => vec![]
		};

		// If we still don't have addresses, try to get them from the active connection
		if addresses.is_empty() {
			// Check if there's an active connection we can get addresses from
			// This would require access to the endpoint or connection info
			self.logger.warn(&format!("No addresses available for device {} during pairing completion", device_id)).await;
		}

		let state = DeviceState::Paired {
			info: info.clone(),
			session_keys: session_keys.clone(),
			paired_at: Utc::now(),
		};

		self.devices.insert(device_id, state);

		// Persist the paired device for future reconnection
		if let Err(e) = self.persistence.add_paired_device(device_id, info.clone(), session_keys.clone(), addresses).await {
			self.logger.warn(&format!("⚠️ Failed to persist paired device {}: {}", device_id, e)).await;
			// Continue anyway - pairing succeeded even if persistence failed
		} else {
			self.logger.debug(&format!("✅ Persisted paired device: {}", device_id)).await;
		}

		Ok(())
	}

	/// Mark device as connected
	pub async fn mark_connected(&mut self, device_id: Uuid, connection: DeviceConnection) -> Result<()> {
		let current_state = self
			.devices
			.get(&device_id)
			.ok_or_else(|| NetworkingError::DeviceNotFound(device_id))?;

		let (info, session_keys): (DeviceInfo, super::SessionKeys) = match current_state {
			DeviceState::Paired { info, session_keys, .. } => (info.clone(), session_keys.clone()),
			DeviceState::Disconnected { info, .. } => {
				// For disconnected devices, we need to find their session keys from a previous state
				// This is a limitation - we should store session keys with disconnected devices too
				return Err(NetworkingError::Protocol(
					"Cannot connect disconnected device without session keys".to_string(),
				));
			}
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

		// Extract addresses before moving connection
		let addresses: Vec<String> = connection.addresses.iter().map(|addr| addr.to_string()).collect();

		let state = DeviceState::Connected {
			info,
			connection,
			session_keys,
			connected_at: Utc::now(),
		};

		self.devices.insert(device_id, state);

		// Update persistence - device connected successfully with current addresses
		if let Err(e) = self.persistence.update_device_connection(device_id, true, Some(addresses)).await {
			self.logger.warn(&format!("⚠️ Failed to update device connection status {}: {}", device_id, e)).await;
		}

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

		let info = match current_state {
			DeviceState::Connected { info, .. } => info.clone(),
			DeviceState::Paired { info, .. } => info.clone(),
			_ => {
				return Err(NetworkingError::Protocol(
					"Cannot disconnect device that isn't connected".to_string(),
				));
			}
		};

		let state = DeviceState::Disconnected {
			info,
			last_seen: Utc::now(),
			reason,
		};

		self.devices.insert(device_id, state);

		// Update persistence - device disconnected
		if let Err(e) = self.persistence.update_device_connection(device_id, false, None).await {
			self.logger.warn(&format!("⚠️ Failed to update device disconnection status {}: {}", device_id, e)).await;
		}

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
			// Clean up mappings
			match &state {
				DeviceState::Discovered { node_id, .. } | DeviceState::Pairing { node_id, .. } => {
					self.node_to_device.remove(node_id);
				}
				DeviceState::Pairing { session_id, .. } => {
					self.session_to_device.remove(session_id);
				}
				_ => {}
			}
		}

		Ok(())
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
			_ => {
				// Device not found or not paired/connected
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

		// TODO: Get actual values from device manager or system
		Ok(DeviceInfo {
			device_id,
			device_name,
			device_type: super::DeviceType::Desktop, // TODO: Detect actual device type
			os_version: std::env::consts::OS.to_string(),
			app_version: env!("CARGO_PKG_VERSION").to_string(),
			network_fingerprint:
				crate::service::network::utils::identity::NetworkFingerprint {
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
				DeviceState::Pairing { started_at, session_id, .. } => {
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
	pub async fn set_device_connected(&mut self, device_id: Uuid, node_id: NodeId, addresses: Vec<String>) -> Result<()> {
		self.logger.info(&format!("Setting device {} as connected with addresses: {:?}", device_id, addresses)).await; // <-- Add this line

		// Update the node_to_device mapping
		self.node_to_device.insert(node_id, device_id);

		// Get the current device state to preserve info
		if let Some(current_state) = self.devices.get(&device_id) {
			match current_state {
				DeviceState::Paired { info, session_keys, .. } => {
					let state = DeviceState::Connected {
						info: info.clone(),
						session_keys: session_keys.clone(),
						connected_at: Utc::now(),
						connection: DeviceConnection {
							addresses: addresses.clone(),
							latency_ms: None,
							rx_bytes: 0,
							tx_bytes: 0,
						},
					};
					self.devices.insert(device_id, state);

					// Update persisted device with new addresses for future reconnection
					if !addresses.is_empty() {
						if let Err(e) = self.persistence.update_device_connection(
							device_id,
							true, // connected
							Some(addresses),
						).await {
							self.logger.warn(&format!("Failed to update device connection info: {}", e)).await;
						}
					}
				}
				DeviceState::Connected { info, session_keys, connection, .. } => {
					// Device is already connected, just update the addresses if provided
					if !addresses.is_empty() {
						let updated_connection = DeviceConnection {
							addresses: addresses.clone(),
							latency_ms: connection.latency_ms,
							rx_bytes: connection.rx_bytes,
							tx_bytes: connection.tx_bytes,
						};

						let state = DeviceState::Connected {
							info: info.clone(),
							session_keys: session_keys.clone(),
							connected_at: Utc::now(),
							connection: updated_connection,
						};
						self.devices.insert(device_id, state);

						// Update persisted device with new addresses
						if let Err(e) = self.persistence.update_device_connection(
							device_id,
							true, // connected
							Some(addresses),
						).await {
							self.logger.warn(&format!("Failed to update device connection info: {}", e)).await;
						}
					}
					// If already connected and no new addresses, it's a no-op
					self.logger.debug(&format!("Device {} already connected, updating node mapping", device_id)).await;
				}
				_ => {
					return Err(NetworkingError::Protocol(
						"Device must be paired before connecting".to_string(),
					));
				}
			}
		} else {
			return Err(NetworkingError::DeviceNotFound(device_id));
		}

		Ok(())
	}
}
