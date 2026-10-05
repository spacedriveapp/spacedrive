//! Library sync setup action

use super::{input::LibrarySyncSetupInput, output::LibrarySyncSetupOutput, LibrarySyncAction};
use crate::infra::action::{error::ActionError, CoreAction};
use crate::service::network::protocol::library_messages::{DeviceRecord, LibraryMessage};
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
		if input.leader_device_id != input.local_device_id
			&& input.leader_device_id != input.remote_device_id
		{
			return Err("Leader device must be either local or remote device".to_string());
		}
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
}

impl LibrarySyncSetupAction {
	/// Store the remote device's record in a local library.
	///
	/// The record comes back in the RegisterDeviceResponse, so both device
	/// tables are complete when the setup command returns instead of relying
	/// on a second request the peer sends later. Insert or update, with the
	/// same slug collision rule the inbound handler applies.
	async fn store_remote_device(
		local_library: &Arc<crate::library::Library>,
		record: DeviceRecord,
	) -> Result<(), ActionError> {
		use crate::infra::db::entities;
		use chrono::Utc;
		use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

		let db = local_library.db().conn();
		let existing = entities::device::Entity::find()
			.filter(entities::device::Column::Uuid.eq(record.device_id))
			.one(db)
			.await
			.map_err(|e| ActionError::Internal(format!("Database error: {}", e)))?;

		let gpu_models = record.gpu_models.map(|g| serde_json::json!(g));

		if let Some(existing) = existing {
			let mut model: entities::device::ActiveModel = existing.into();
			model.name = Set(record.device_name);
			model.os = Set(record.os_name);
			model.os_version = Set(record.os_version);
			model.hardware_model = Set(record.hardware_model);
			model.cpu_model = Set(record.cpu_model);
			model.cpu_architecture = Set(record.cpu_architecture);
			model.cpu_cores_physical = Set(record.cpu_cores_physical);
			model.cpu_cores_logical = Set(record.cpu_cores_logical);
			model.cpu_frequency_mhz = Set(record.cpu_frequency_mhz);
			model.memory_total_bytes = Set(record.memory_total_bytes);
			model.form_factor = Set(record.form_factor);
			model.manufacturer = Set(record.manufacturer);
			model.gpu_models = Set(gpu_models);
			model.boot_disk_type = Set(record.boot_disk_type);
			model.boot_disk_capacity_bytes = Set(record.boot_disk_capacity_bytes);
			model.swap_total_bytes = Set(record.swap_total_bytes);
			model.last_seen_at = Set(Utc::now());
			model.updated_at = Set(Utc::now());
			model
				.update(db)
				.await
				.map_err(|e| ActionError::Internal(format!("Failed to update device: {}", e)))?;
			return Ok(());
		}

		let existing_slugs: Vec<String> = entities::device::Entity::find()
			.all(db)
			.await
			.map_err(|e| ActionError::Internal(format!("Database error: {}", e)))?
			.into_iter()
			.map(|d| d.slug)
			.collect();
		let slug =
			crate::library::Library::ensure_unique_slug(&record.device_slug, &existing_slugs);

		entities::device::ActiveModel {
			id: sea_orm::ActiveValue::NotSet,
			uuid: Set(record.device_id),
			name: Set(record.device_name),
			slug: Set(slug),
			os: Set(record.os_name),
			os_version: Set(record.os_version),
			hardware_model: Set(record.hardware_model),
			cpu_model: Set(record.cpu_model),
			cpu_architecture: Set(record.cpu_architecture),
			cpu_cores_physical: Set(record.cpu_cores_physical),
			cpu_cores_logical: Set(record.cpu_cores_logical),
			cpu_frequency_mhz: Set(record.cpu_frequency_mhz),
			memory_total_bytes: Set(record.memory_total_bytes),
			form_factor: Set(record.form_factor),
			manufacturer: Set(record.manufacturer),
			gpu_models: Set(gpu_models),
			boot_disk_type: Set(record.boot_disk_type),
			boot_disk_capacity_bytes: Set(record.boot_disk_capacity_bytes),
			swap_total_bytes: Set(record.swap_total_bytes),
			network_addresses: Set(serde_json::json!([])),
			is_online: Set(true),
			last_seen_at: Set(Utc::now()),
			capabilities: Set(serde_json::json!({
				"indexing": true,
				"p2p": true,
				"volume_detection": true
			})),
			created_at: Set(Utc::now()),
			updated_at: Set(Utc::now()),
			sync_enabled: Set(true),
		}
		.insert(db)
		.await
		.map_err(|e| ActionError::Internal(format!("Failed to insert device: {}", e)))?;

		info!(
			"Registered remote device {} in library {}",
			record.device_id,
			local_library.id()
		);
		Ok(())
	}

	/// Send our registration to the remote device and store its record from
	/// the response. Returns false when the remote did not register us.
	async fn exchange_device_records(
		&self,
		context: &Arc<crate::context::CoreContext>,
		networking: &Arc<crate::service::network::NetworkingService>,
		local_library: &Arc<crate::library::Library>,
		needs_initial_state: bool,
	) -> Result<bool, ActionError> {
		let library_id = local_library.id();
		let local_device = context
			.device_manager
			.to_device()
			.map_err(|e| ActionError::Internal(format!("Failed to get device info: {}", e)))?;
		let local_device_slug = context
			.device_manager
			.slug_for_library(library_id)
			.map_err(|e| ActionError::Internal(format!("Failed to get device slug: {}", e)))?;

		let register_request = LibraryMessage::RegisterDeviceRequest {
			request_id: Uuid::new_v4(),
			library_id: Some(library_id),
			device_id: self.input.local_device_id,
			device_name: local_device.name,
			device_slug: local_device_slug,
			os_name: local_device.os.to_string(),
			os_version: local_device.os_version,
			hardware_model: local_device.hardware_model,
			cpu_model: local_device.cpu_model,
			cpu_architecture: local_device.cpu_architecture,
			cpu_cores_physical: local_device.cpu_cores_physical,
			cpu_cores_logical: local_device.cpu_cores_logical,
			cpu_frequency_mhz: local_device.cpu_frequency_mhz,
			memory_total_bytes: local_device.memory_total_bytes,
			form_factor: local_device.form_factor.map(|f| f.to_string()),
			manufacturer: local_device.manufacturer,
			gpu_models: local_device.gpu_models,
			boot_disk_type: local_device.boot_disk_type,
			boot_disk_capacity_bytes: local_device.boot_disk_capacity_bytes,
			swap_total_bytes: local_device.swap_total_bytes,
			needs_initial_state,
		};

		match networking
			.send_library_request(self.input.remote_device_id, register_request)
			.await
		{
			Ok(LibraryMessage::RegisterDeviceResponse {
				success: true,
				device,
				..
			}) => {
				info!("Successfully registered local device on remote device");
				match device {
					Some(record) => {
						Self::store_remote_device(local_library, record).await?;
						Ok(true)
					}
					None => {
						warn!("Remote device registered us but did not return its own record");
						Ok(false)
					}
				}
			}
			Ok(LibraryMessage::RegisterDeviceResponse {
				success: false,
				message,
				..
			}) => {
				warn!(
					"Remote device failed to register local device: {}",
					message
						.clone()
						.unwrap_or_else(|| "Unknown error".to_string())
				);
				Err(ActionError::Internal(format!(
					"Remote device refused registration: {}",
					message.unwrap_or_else(|| "Unknown error".to_string())
				)))
			}
			Ok(_) => Err(ActionError::Internal(
				"Unexpected response from remote device for register request".to_string(),
			)),
			Err(e) => Err(ActionError::Internal(format!(
				"Failed to send register request to remote device: {}",
				e
			))),
		}
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
		// Get full device information including hardware specs
		let local_device = context
			.device_manager
			.to_device()
			.map_err(|e| ActionError::Internal(format!("Failed to get device info: {}", e)))?;

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
			requesting_device_name: local_device.name,
			requesting_device_slug: local_device_slug,
			requesting_device_os: local_device.os.to_string(),
			requesting_device_os_version: local_device.os_version,
			requesting_device_hardware_model: local_device.hardware_model,
			requesting_device_cpu_model: local_device.cpu_model,
			requesting_device_cpu_architecture: local_device.cpu_architecture,
			requesting_device_cpu_cores_physical: local_device.cpu_cores_physical,
			requesting_device_cpu_cores_logical: local_device.cpu_cores_logical,
			requesting_device_cpu_frequency_mhz: local_device.cpu_frequency_mhz,
			requesting_device_memory_total_bytes: local_device.memory_total_bytes,
			requesting_device_form_factor: local_device.form_factor.map(|f| f.to_string()),
			requesting_device_manufacturer: local_device.manufacturer,
			requesting_device_gpu_models: local_device.gpu_models,
			requesting_device_boot_disk_type: local_device.boot_disk_type,
			requesting_device_boot_disk_capacity_bytes: local_device.boot_disk_capacity_bytes,
			requesting_device_swap_total_bytes: local_device.swap_total_bytes,
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

				// Both device tables fill in one round trip: the remote stores
				// us, and its own record comes back in the response.
				let devices_registered = self
					.exchange_device_records(&context, &networking, local_library, false)
					.await?;

				Ok(LibrarySyncSetupOutput {
					success: true,
					local_library_id: library_id,
					remote_library_id: Some(library_id),
					devices_registered,
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

		let networking = context
			.get_networking()
			.await
			.ok_or_else(|| ActionError::Internal("Networking not available".to_string()))?;

		// The new copy holds nothing. The flag tells the remote device it must
		// have state to offer, and its record in the response gives the sync
		// service a member to backfill from. A join whose registration failed
		// leaves a library with no partner, so that is a failure, not a warning.
		let devices_registered = self
			.exchange_device_records(&context, &networking, &local_library, true)
			.await?;
		if !devices_registered {
			return Err(ActionError::Internal(
				"Remote device registered this device but did not return its own record; the joined library has no member to sync from".to_string(),
			));
		}

		Ok(LibrarySyncSetupOutput {
			success: true,
			local_library_id: remote_library_id,
			remote_library_id: Some(remote_library_id),
			devices_registered,
			message: format!(
				"Successfully joined remote library '{}'",
				remote_library_name
			),
		})
	}
}

crate::register_core_action!(LibrarySyncSetupAction, "network.sync_setup");
