//! Library sync setup action

use super::{input::LibrarySyncSetupInput, output::LibrarySyncSetupOutput, LibrarySyncAction};
use crate::infra::action::{error::ActionError, CoreAction};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

pub struct LibrarySyncSetupAction {
	input: LibrarySyncSetupInput,
}

impl CoreAction for LibrarySyncSetupAction {
	type Input = LibrarySyncSetupInput;
	type Output = LibrarySyncSetupOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<crate::context::CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Validate that networking is available
		let networking = context
			.get_networking()
			.await
			.ok_or_else(|| ActionError::Internal("Networking not initialized".to_string()))?;

		// Validate that remote device is paired
		let device_registry = networking.device_registry();
		let registry = device_registry.read().await;

		let device_state = registry
			.get_device_state(self.input.remote_device_id)
			.ok_or_else(|| ActionError::Validation {
				field: "remote_device_id".to_string(),
				message: "Device not found".to_string(),
			})?;

		// Verify device is paired or connected
		match device_state {
			crate::service::network::device::DeviceState::Paired { .. }
			| crate::service::network::device::DeviceState::Connected { .. } => {}
			_ => {
				return Err(ActionError::Validation {
					field: "remote_device_id".to_string(),
					message: "Device must be paired before setting up library sync".to_string(),
				});
			}
		}

		drop(registry);

		// Get library manager
		let library_manager = context.libraries().await;

		// Validate local library exists
		let local_library = library_manager
			.get_library(self.input.local_library_id)
			.await
			.ok_or_else(|| ActionError::Validation {
				field: "local_library_id".to_string(),
				message: "Local library not found".to_string(),
			})?;

		// Execute based on action type
		match &self.input.action {
			LibrarySyncAction::RegisterOnly => {
				self.execute_register_only(context.clone(), &local_library)
					.await
			}
			LibrarySyncAction::MergeIntoLocal { remote_library_id } => {
				// Future implementation
				Err(ActionError::Internal(
					"MergeIntoLocal not yet implemented - requires sync system".to_string(),
				))
			}
			LibrarySyncAction::MergeIntoRemote { local_library_id } => {
				// Future implementation
				Err(ActionError::Internal(
					"MergeIntoRemote not yet implemented - requires sync system".to_string(),
				))
			}
			LibrarySyncAction::CreateShared {
				leader_device_id,
				name,
			} => {
				// Future implementation
				Err(ActionError::Internal(
					"CreateShared not yet implemented - requires sync system".to_string(),
				))
			}
		}
	}

	fn action_kind(&self) -> &'static str {
		"network.sync_setup"
	}

	// DEPRICATED: Sync no longer requires a leader device
	async fn validate(&self, context: Arc<crate::context::CoreContext>) -> Result<(), ActionError> {
		// Validate leader device is one of the two devices
		if self.input.leader_device_id != self.input.local_device_id
			&& self.input.leader_device_id != self.input.remote_device_id
		{
			return Err(ActionError::Validation {
				field: "leader_device_id".to_string(),
				message: "Leader device must be either local or remote device".to_string(),
			});
		}

		Ok(())
	}
}

impl LibrarySyncSetupAction {
	/// Execute RegisterOnly action - just register devices in each other's libraries
	async fn execute_register_only(
		&self,
		context: Arc<crate::context::CoreContext>,
		local_library: &Arc<crate::library::Library>,
	) -> Result<LibrarySyncSetupOutput, ActionError> {
		info!(
			"Registering devices for library sync: local_device={}, remote_device={}, library={}",
			self.input.local_device_id, self.input.remote_device_id, self.input.local_library_id
		);

		// Get networking to access device info
		let networking = context
			.get_networking()
			.await
			.ok_or_else(|| ActionError::Internal("Networking not available".to_string()))?;

		let device_registry = networking.device_registry();
		let registry = device_registry.read().await;

		// Get remote device info
		let remote_device_info = match registry.get_device_state(self.input.remote_device_id) {
			Some(crate::service::network::device::DeviceState::Paired { info, .. }) => info.clone(),
			Some(crate::service::network::device::DeviceState::Connected { info, .. }) => {
				info.clone()
			}
			_ => {
				return Err(ActionError::Internal(format!(
					"Could not get info for device {}",
					self.input.remote_device_id
				)));
			}
		};

		drop(registry);

		// Register both local and remote devices in the local library
		use crate::infra::db::entities;
		use chrono::Utc;
		use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

		let db = local_library.db();

		// Extract device OS info from DeviceType if available
		let device_os = match &remote_device_info.device_type {
			crate::service::network::device::DeviceType::Desktop => "Desktop",
			crate::service::network::device::DeviceType::Laptop => "Laptop",
			crate::service::network::device::DeviceType::Mobile => "Mobile",
			crate::service::network::device::DeviceType::Server => "Server",
			crate::service::network::device::DeviceType::Other(s) => s.as_str(),
		};

		// Check if remote device already exists in this library
		let existing_device = entities::device::Entity::find()
			.filter(entities::device::Column::Uuid.eq(self.input.remote_device_id))
			.one(db.conn())
			.await
			.map_err(|e| ActionError::Internal(format!("Database error: {}", e)))?;

		if existing_device.is_none() {
			// Register remote device in local library
			let device_model = entities::device::ActiveModel {
				id: sea_orm::ActiveValue::NotSet,
				uuid: Set(self.input.remote_device_id),
				name: Set(remote_device_info.device_name.clone()),
				os: Set(device_os.to_string()),
				os_version: Set(Some(remote_device_info.os_version.clone())),
				hardware_model: Set(None), // Not available in DeviceInfo
				network_addresses: Set(serde_json::json!([])),
				is_online: Set(false),
				last_seen_at: Set(Utc::now()),
				capabilities: Set(serde_json::json!({
					"indexing": true,
					"p2p": true,
					"volume_detection": true
				})),
				created_at: Set(Utc::now()),
				updated_at: Set(Utc::now()),
				sync_enabled: Set(true), // Enable sync for registered devices
				last_sync_at: Set(None),
			};

			device_model
				.insert(db.conn())
				.await
				.map_err(|e| ActionError::Internal(format!("Failed to insert device: {}", e)))?;

			info!(
				"Registered remote device {} in library {}",
				self.input.remote_device_id, self.input.local_library_id
			);
		} else {
			info!(
				"Remote device {} already registered in library {}",
				self.input.remote_device_id, self.input.local_library_id
			);
		}

		// Send request to remote device to register local device in their libraries
		use crate::service::network::protocol::library_messages::LibraryMessage;

		let local_device_config = context
			.device_manager
			.config()
			.map_err(|e| ActionError::Internal(format!("Failed to get device config: {}", e)))?;

		let request = LibraryMessage::RegisterDeviceRequest {
			request_id: Uuid::new_v4(),
			library_id: self.input.remote_library_id,
			device_id: self.input.local_device_id,
			device_name: local_device_config.name.clone(),
			os_name: local_device_config.os.to_string(),
			os_version: None, // Not available in DeviceConfig
			hardware_model: local_device_config.hardware_model.clone(),
		};

		// Send request (best effort - don't fail if remote registration fails)
		match networking
			.send_library_request(self.input.remote_device_id, request)
			.await
		{
			Ok(LibraryMessage::RegisterDeviceResponse {
				request_id: _,
				success,
				message,
			}) => {
				if success {
					info!(
						"Successfully registered local device on remote device in library {:?}",
						self.input.remote_library_id
					);
				} else {
					warn!(
						"Remote device failed to register local device: {}",
						message.unwrap_or_else(|| "Unknown error".to_string())
					);
				}
			}
			Ok(_) => {
				warn!("Unexpected response from remote device for register request");
			}
			Err(e) => {
				warn!(
					"Failed to send register request to remote device: {}. Local registration succeeded.",
					e
				);
			}
		}

		Ok(LibrarySyncSetupOutput {
			success: true,
			local_library_id: self.input.local_library_id,
			remote_library_id: self.input.remote_library_id,
			devices_registered: true,
			message: "Devices successfully registered for library access".to_string(),
		})
	}
}

crate::register_core_action!(LibrarySyncSetupAction, "network.sync_setup");
