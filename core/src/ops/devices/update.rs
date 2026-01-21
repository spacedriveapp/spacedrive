//! Update device configuration action

use crate::{
	context::CoreContext,
	device::DeviceConfig,
	infra::action::{error::ActionError, CoreAction, ValidationResult},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use tracing::info;

/// Input for updating device configuration
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateDeviceInput {
	/// Device name
	#[serde(skip_serializing_if = "Option::is_none")]
	pub name: Option<String>,

	/// Device slug
	#[serde(skip_serializing_if = "Option::is_none")]
	pub slug: Option<String>,
}

/// Output from updating device configuration
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateDeviceOutput {
	/// Updated device name
	pub name: String,
	/// Updated device slug
	pub slug: String,
}

/// Action to update device configuration
pub struct UpdateDeviceAction {
	input: UpdateDeviceInput,
}

impl CoreAction for UpdateDeviceAction {
	type Input = UpdateDeviceInput;
	type Output = UpdateDeviceOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		// Validate that at least one field is being updated
		if input.name.is_none() && input.slug.is_none() {
			return Err("At least one field (name or slug) must be provided".to_string());
		}

		// Validate name if provided
		if let Some(ref name) = input.name {
			if name.trim().is_empty() {
				return Err("Device name cannot be empty".to_string());
			}
			if name.len() > 100 {
				return Err("Device name cannot exceed 100 characters".to_string());
			}
		}

		// Validate slug if provided
		if let Some(ref slug) = input.slug {
			if slug.trim().is_empty() {
				return Err("Device slug cannot be empty".to_string());
			}
			if slug.len() > 50 {
				return Err("Device slug cannot exceed 50 characters".to_string());
			}
			// Validate slug format (alphanumeric + hyphens only)
			if !slug.chars().all(|c| c.is_alphanumeric() || c == '-') {
				return Err("Device slug can only contain letters, numbers, and hyphens".to_string());
			}
		}

		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Load current device config
		let mut device_config = DeviceConfig::load_from(&context.data_dir)
			.map_err(|e| ActionError::Internal(format!("Failed to load device config: {}", e)))?;

		// Apply updates
		if let Some(name) = self.input.name {
			info!("Updating device name: {} -> {}", device_config.name, name);
			device_config.name = name;
		}

		if let Some(slug) = self.input.slug {
			info!("Updating device slug: {} -> {}", device_config.slug, slug);
			device_config.slug = slug;
		}

		// Save updated config
		device_config
			.save_to(&context.data_dir)
			.map_err(|e| ActionError::Internal(format!("Failed to save device config: {}", e)))?;

		info!("Device configuration updated successfully");

		Ok(UpdateDeviceOutput {
			name: device_config.name,
			slug: device_config.slug,
		})
	}

	async fn validate(
		&self,
		_context: Arc<CoreContext>,
	) -> Result<ValidationResult, ActionError> {
		// Basic validation is done in from_input
		Ok(ValidationResult::Success { metadata: None })
	}

	fn action_kind(&self) -> &'static str {
		"device.update"
	}
}

crate::register_core_action!(UpdateDeviceAction, "device.update");
