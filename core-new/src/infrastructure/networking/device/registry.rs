//! Device registry for centralized state management

use super::{DeviceConnection, DeviceInfo, DeviceState, DevicePersistence, PersistedPairedDevice, SessionKeys, TrustLevel};
use crate::device::DeviceManager;
use crate::infrastructure::networking::{NetworkingError, Result};
use chrono::{DateTime, Utc};
use libp2p::{Multiaddr, PeerId};
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

	/// Map of peer ID to device ID for quick lookup
	peer_to_device: HashMap<PeerId, Uuid>,

	/// Map of session ID to device ID for pairing lookup
	session_to_device: HashMap<Uuid, Uuid>,

	/// Persistence manager for paired devices
	persistence: DevicePersistence,
}

impl DeviceRegistry {
	/// Create a new device registry
	pub fn new(device_manager: Arc<DeviceManager>, data_dir: impl AsRef<Path>) -> Result<Self> {
		let persistence = DevicePersistence::new(data_dir)?;
		
		Ok(Self {
			device_manager,
			devices: HashMap::new(),
			peer_to_device: HashMap::new(),
			session_to_device: HashMap::new(),
			persistence,
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

	/// Add a discovered peer
	pub fn add_discovered_peer(
		&mut self,
		device_id: Uuid,
		peer_id: PeerId,
		addresses: Vec<Multiaddr>,
	) {
		let state = DeviceState::Discovered {
			peer_id,
			addresses,
			discovered_at: Utc::now(),
		};

		self.devices.insert(device_id, state);
		self.peer_to_device.insert(peer_id, device_id);
	}

	/// Start pairing process for a device
	pub fn start_pairing(
		&mut self,
		device_id: Uuid,
		peer_id: PeerId,
		session_id: Uuid,
	) -> Result<()> {
		let state = DeviceState::Pairing {
			peer_id,
			session_id,
			started_at: Utc::now(),
		};

		self.devices.insert(device_id, state);
		self.peer_to_device.insert(peer_id, device_id);
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
		// Parse peer ID from network fingerprint
		let addresses = if let Ok(peer_id) = info.network_fingerprint.peer_id.parse::<libp2p::PeerId>() {
			// Add peer-to-device mapping so device can be found for messaging
			self.peer_to_device.insert(peer_id, device_id);
			println!("🔗 Added peer-to-device mapping: {} -> {}", peer_id, device_id);
			
			// Get current addresses from discovered state if available
			match self.devices.get(&device_id) {
				Some(DeviceState::Discovered { addresses, .. }) => {
					addresses.iter().map(|addr| addr.to_string()).collect()
				}
				Some(DeviceState::Pairing { .. }) => {
					vec![] // Pairing state doesn't have addresses
				}
				_ => vec![]
			}
		} else {
			println!("⚠️ Failed to parse peer ID from network fingerprint: {}", info.network_fingerprint.peer_id);
			vec![]
		};

		let state = DeviceState::Paired {
			info: info.clone(),
			session_keys: session_keys.clone(),
			paired_at: Utc::now(),
		};

		self.devices.insert(device_id, state);

		// Persist the paired device for future reconnection
		if let Err(e) = self.persistence.add_paired_device(device_id, info.clone(), session_keys.clone(), addresses).await {
			eprintln!("⚠️ Failed to persist paired device {}: {}", device_id, e);
			// Continue anyway - pairing succeeded even if persistence failed
		} else {
			println!("✅ Persisted paired device: {}", device_id);
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
			DeviceState::Discovered { peer_id, .. } => {
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
			info,
			connection,
			session_keys,
			connected_at: Utc::now(),
		};

		self.devices.insert(device_id, state);

		// Update persistence - device connected successfully
		if let Err(e) = self.persistence.update_device_connection(device_id, true, None).await {
			eprintln!("⚠️ Failed to update device connection status {}: {}", device_id, e);
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
			eprintln!("⚠️ Failed to update device disconnection status {}: {}", device_id, e);
		}

		Ok(())
	}

	/// Get device state by device ID
	pub fn get_device_state(&self, device_id: Uuid) -> Option<&DeviceState> {
		self.devices.get(&device_id)
	}

	/// Get device ID by peer ID
	pub fn get_device_by_peer(&self, peer_id: PeerId) -> Option<Uuid> {
		self.peer_to_device.get(&peer_id).copied()
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
				DeviceState::Discovered { peer_id, .. } | DeviceState::Pairing { peer_id, .. } => {
					self.peer_to_device.remove(peer_id);
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
	pub fn get_peer_by_device(&self, device_id: Uuid) -> Option<PeerId> {
		// Look through peer_to_device map in reverse
		for (peer_id, &dev_id) in &self.peer_to_device {
			if dev_id == device_id {
				println!("🔗 REGISTRY_DEBUG: Found peer {} for device {}", peer_id, device_id);
				return Some(*peer_id);
			}
		}
		println!("⚠️ REGISTRY_DEBUG: No peer found for device {}. Available mappings:", device_id);
		for (peer_id, mapped_device_id) in &self.peer_to_device {
			println!("   {} -> {}", peer_id, mapped_device_id);
		}
		None
	}

	/// Get session keys for a device
	pub fn get_session_keys(&self, device_id: Uuid) -> Option<super::SessionKeys> {
		match self.devices.get(&device_id) {
			Some(DeviceState::Paired { session_keys, .. }) => {
				println!("🔑 REGISTRY_DEBUG: Found session keys for paired device {}", device_id);
				Some(session_keys.clone())
			}
			Some(DeviceState::Connected { session_keys, .. }) => {
				println!("🔑 REGISTRY_DEBUG: Found session keys for connected device {}", device_id);
				Some(session_keys.clone())
			}
			_ => {
				println!("🔑 REGISTRY_DEBUG: Device {} not found or not paired/connected", device_id);
				None
			}
		}
	}

	/// Get all currently connected peer IDs
	pub fn get_connected_peers(&self) -> Vec<PeerId> {
		self.peer_to_device.keys().cloned().collect()
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
				crate::infrastructure::networking::utils::identity::NetworkFingerprint {
					peer_id: "placeholder".to_string(), // Will be filled in by caller
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
}
