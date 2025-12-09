//! Library sync setup action

use super::{input::LibrarySyncSetupInput, output::LibrarySyncSetupOutput, LibrarySyncAction};
use crate::infra::action::{error::ActionError, CoreAction};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Clone)]
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
			LibrarySyncAction::ShareLocalLibrary { library_name } => {
				self.execute_share_local(context.clone(), &local_library, library_name.clone())
					.await
			}
			LibrarySyncAction::JoinRemoteLibrary {
				remote_library_id,
				remote_library_name,
			} => {
				self.execute_join_remote(
					context.clone(),
					&local_library,
					*remote_library_id,
					remote_library_name.clone(),
				)
				.await
			}
			LibrarySyncAction::MergeLibraries { .. } => Err(ActionError::Internal(
				"MergeLibraries not yet implemented - requires full sync system".to_string(),
			)),
		}
	}

	fn action_kind(&self) -> &'static str {
		"network.sync_setup"
	}

	// DEPRICATED: Sync no longer requires a leader device
	async fn validate(
		&self,
		context: Arc<crate::context::CoreContext>,
	) -> Result<crate::infra::action::ValidationResult, ActionError> {
		// Validate leader device is one of the two devices
		if self.input.leader_device_id != self.input.local_device_id
			&& self.input.leader_device_id != self.input.remote_device_id
		{
			return Err(ActionError::Validation {
				field: "leader_device_id".to_string(),
				message: "Leader device must be either local or remote device".to_string(),
			});
		}

		Ok(crate::infra::action::ValidationResult::Success)
	}
}

impl LibrarySyncSetupAction {
	/// Register remote device in local library using its library-specific slug
	/// The slug should come from the remote device (either from CreateSharedLibraryResponse
	/// or from the remote device's DeviceInfo which includes library overrides)
	async fn register_remote_device_in_library(
		&self,
		context: &Arc<crate::context::CoreContext>,
		local_library: &Arc<crate::library::Library>,
		remote_device_id: Uuid,
		remote_device_slug: String,
	) -> Result<crate::infra::action::ValidationResult, ActionError> {
		use crate::infra::db::entities;
		use chrono::Utc;
		use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

		// Get networking to access device info
		let networking = context
			.get_networking()
			.await
			.ok_or_else(|| ActionError::Internal("Networking not available".to_string()))?;

		let device_registry = networking.device_registry();
		let registry = device_registry.read().await;

		// Get remote device info
		let remote_device_info = match registry.get_device_state(remote_device_id) {
			Some(crate::service::network::device::DeviceState::Paired { info, .. }) => info.clone(),
			Some(crate::service::network::device::DeviceState::Connected { info, .. }) => {
				info.clone()
			}
			_ => {
				return Err(ActionError::Internal(format!(
					"Could not get info for device {}",
					remote_device_id
				)));
			}
		};

		drop(registry);

		let db = local_library.db();

		// Check if remote device already exists
		let existing_device = entities::device::Entity::find()
			.filter(entities::device::Column::Uuid.eq(remote_device_id))
			.one(db.conn())
			.await
			.map_err(|e| ActionError::Internal(format!("Database error: {}", e)))?;

		if existing_device.is_none() {
			// Extract device OS info
			let device_os = match &remote_device_info.device_type {
				crate::service::network::device::DeviceType::Desktop => "Desktop",
				crate::service::network::device::DeviceType::Laptop => "Laptop",
				crate::service::network::device::DeviceType::Mobile => "Mobile",
				crate::service::network::device::DeviceType::Server => "Server",
				crate::service::network::device::DeviceType::Other(s) => s.as_str(),
			};

			// Register remote device with its library-specific slug
			let device_model = entities::device::ActiveModel {
				id: sea_orm::ActiveValue::NotSet,
				uuid: Set(remote_device_id),
				name: Set(remote_device_info.device_name.clone()),
				slug: Set(remote_device_slug.clone()),
				os: Set(device_os.to_string()),
				os_version: Set(Some(remote_device_info.os_version.clone())),
				hardware_model: Set(None),
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
				sync_enabled: Set(true),
				last_sync_at: Set(None),
			};

			device_model
				.insert(db.conn())
				.await
				.map_err(|e| ActionError::Internal(format!("Failed to insert device: {}", e)))?;

			info!(
				"Registered remote device {} in library {} with slug '{}'",
				remote_device_id,
				local_library.id(),
				remote_device_slug
			);
		}

		Ok(crate::infra::action::ValidationResult::Success)
	}

	/// Execute ShareLocalLibrary action - share local library to remote device
	async fn execute_share_local(
		&self,
		context: Arc<crate::context::CoreContext>,
		local_library: &Arc<crate::library::Library>,
		_library_name: String,
	) -> Result<LibrarySyncSetupOutput, ActionError> {
		info!(
			"Sharing local library: local_library={}, remote_device={}",
			self.input.local_library_id, self.input.remote_device_id
		);

		let library_id = local_library.id();
		let library_name = local_library.name().await;
		let config = local_library.config().await;

		// Get networking
		let networking = context
			.get_networking()
			.await
			.ok_or_else(|| ActionError::Internal("Networking not available".to_string()))?;

		// Send CreateSharedLibraryRequest to remote device
		use crate::service::network::protocol::library_messages::LibraryMessage;

		let local_device_config = context
			.device_manager
			.config()
			.map_err(|e| ActionError::Internal(format!("Failed to get device config: {}", e)))?;

		// Get library-specific slug for this device
		let local_device_slug = context
			.device_manager
			.slug_for_library(library_id)
			.map_err(|e| ActionError::Internal(format!("Failed to get device slug: {}", e)))?;

		let request = LibraryMessage::CreateSharedLibraryRequest {
			request_id: Uuid::new_v4(),
			library_id,
			library_name: library_name.clone(),
			description: config.description.clone(),
			requesting_device_id: self.input.local_device_id,
			requesting_device_name: local_device_config.name.clone(),
			requesting_device_slug: local_device_slug,
		};

		info!(
			"Sending CreateSharedLibraryRequest to remote device: library={}, name={}",
			library_id, library_name
		);

		let response = networking
			.send_library_request(self.input.remote_device_id, request)
			.await
			.map_err(|e| {
				ActionError::Internal(format!("Failed to send create library request: {}", e))
			})?;

		// Check response
		match response {
			LibraryMessage::CreateSharedLibraryResponse {
				request_id: _,
				success: true,
				message,
				device_slug,
			} => {
				info!(
					"Remote device successfully created shared library: {}",
					message.unwrap_or_else(|| "No message".to_string())
				);

				// Get remote device's library-specific slug from response
				let remote_slug = device_slug.ok_or_else(|| {
					ActionError::Internal(
						"Remote device did not return its library-specific slug".to_string(),
					)
				})?;

				info!(
					"Remote device is using slug '{}' in this library",
					remote_slug
				);

				// Register remote device in local library with its resolved slug
				self.register_remote_device_in_library(
					&context,
					local_library,
					self.input.remote_device_id,
					remote_slug,
				)
				.await?;

				// Send request to remote device to register local device
				let networking = context
					.get_networking()
					.await
					.ok_or_else(|| ActionError::Internal("Networking not available".to_string()))?;
				let local_device_config = context.device_manager.config().map_err(|e| {
					ActionError::Internal(format!("Failed to get device config: {}", e))
				})?;

				// Get library-specific slug (uses override if set, otherwise global slug)
				let local_device_slug = context
					.device_manager
					.slug_for_library(library_id)
					.map_err(|e| {
						ActionError::Internal(format!("Failed to get device slug: {}", e))
					})?;

				let register_request = LibraryMessage::RegisterDeviceRequest {
					request_id: Uuid::new_v4(),
					library_id: Some(library_id),
					device_id: self.input.local_device_id,
					device_name: local_device_config.name.clone(),
					device_slug: local_device_slug,
					os_name: local_device_config.os.to_string(),
					os_version: None,
					hardware_model: local_device_config.hardware_model.clone(),
				};

				match networking
					.send_library_request(self.input.remote_device_id, register_request)
					.await
				{
					Ok(LibraryMessage::RegisterDeviceResponse { success: true, .. }) => {
						info!("Successfully registered local device on remote device");
					}
					Ok(LibraryMessage::RegisterDeviceResponse {
						success: false,
						message,
						..
					}) => {
						warn!(
							"Remote device failed to register local device: {}",
							message.unwrap_or_else(|| "Unknown error".to_string())
						);
					}
					Err(e) => {
						warn!("Failed to send register request to remote device: {}", e);
					}
					_ => {
						warn!("Unexpected response from remote device for register request");
					}
				}

				Ok(LibrarySyncSetupOutput {
					success: true,
					local_library_id: library_id,
					remote_library_id: Some(library_id),
					devices_registered: true,
					message: format!(
						"Successfully shared library '{}' to remote device",
						library_name
					),
				})
			}
			LibraryMessage::CreateSharedLibraryResponse {
				request_id: _,
				success: false,
				message,
				..
			} => Err(ActionError::Internal(format!(
				"Remote device failed to create library: {}",
				message.unwrap_or_else(|| "Unknown error".to_string())
			))),
			_ => Err(ActionError::Internal(
				"Unexpected response from remote device".to_string(),
			)),
		}
	}

	/// Execute JoinRemoteLibrary action - join an existing remote library
	async fn execute_join_remote(
		&self,
		context: Arc<crate::context::CoreContext>,
		_local_library: &Arc<crate::library::Library>,
		remote_library_id: Uuid,
		remote_library_name: String,
	) -> Result<LibrarySyncSetupOutput, ActionError> {
		info!(
			"Joining remote library: remote_library={}, remote_device={}",
			remote_library_id, self.input.remote_device_id
		);

		// Get library manager to create the library locally with remote's UUID
		let library_manager = context.libraries().await;

		// Create library with remote's UUID
		let local_library = library_manager
			.create_library_with_id(
				remote_library_id,
				remote_library_name.clone(),
				None,
				context.clone(),
			)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to create local library: {}", e)))?;

		info!(
			"Created local library {} with remote UUID {}",
			remote_library_name, remote_library_id
		);

		// Get remote device's slug from DeviceInfo and register it
		let networking = context
			.get_networking()
			.await
			.ok_or_else(|| ActionError::Internal("Networking not available".to_string()))?;

		let device_registry = networking.device_registry();
		let remote_device_slug = {
			let registry = device_registry.read().await;
			match registry.get_device_state(self.input.remote_device_id) {
				Some(crate::service::network::device::DeviceState::Paired { info, .. })
				| Some(crate::service::network::device::DeviceState::Connected { info, .. }) => {
					info.device_slug.clone()
				}
				_ => {
					return Err(ActionError::Internal(
						"Could not get remote device info".to_string(),
					));
				}
			}
		};

		// Register remote device in the newly created local library
		self.register_remote_device_in_library(
			&context,
			&local_library,
			self.input.remote_device_id,
			remote_device_slug,
		)
		.await?;

		// Send request to remote device to register local device

		let local_device_config = context
			.device_manager
			.config()
			.map_err(|e| ActionError::Internal(format!("Failed to get device config: {}", e)))?;

		// Get library-specific slug (uses override if set, otherwise global slug)
		let local_device_slug = context
			.device_manager
			.slug_for_library(remote_library_id)
			.map_err(|e| ActionError::Internal(format!("Failed to get device slug: {}", e)))?;

		use crate::service::network::protocol::library_messages::LibraryMessage;

		let register_request = LibraryMessage::RegisterDeviceRequest {
			request_id: Uuid::new_v4(),
			library_id: Some(remote_library_id),
			device_id: self.input.local_device_id,
			device_name: local_device_config.name.clone(),
			device_slug: local_device_slug,
			os_name: local_device_config.os.to_string(),
			os_version: None,
			hardware_model: local_device_config.hardware_model.clone(),
		};

		match networking
			.send_library_request(self.input.remote_device_id, register_request)
			.await
		{
			Ok(LibraryMessage::RegisterDeviceResponse { success: true, .. }) => {
				info!("Successfully registered local device on remote device");
			}
			Ok(LibraryMessage::RegisterDeviceResponse {
				success: false,
				message,
				..
			}) => {
				warn!(
					"Remote device failed to register local device: {}",
					message.unwrap_or_else(|| "Unknown error".to_string())
				);
			}
			Err(e) => {
				warn!("Failed to send register request to remote device: {}", e);
			}
			_ => {
				warn!("Unexpected response from remote device for register request");
			}
		}

		Ok(LibrarySyncSetupOutput {
			success: true,
			local_library_id: remote_library_id,
			remote_library_id: Some(remote_library_id),
			devices_registered: true,
			message: format!(
				"Successfully joined remote library '{}'",
				remote_library_name
			),
		})
	}
}

crate::register_core_action!(LibrarySyncSetupAction, "network.sync_setup");
