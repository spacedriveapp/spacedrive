use super::{input::DeviceRevokeInput, output::DeviceRevokeOutput};
use crate::infra::action::{error::ActionError, CoreAction};
use std::sync::Arc;

pub struct DeviceRevokeAction {
	pub device_id: uuid::Uuid,
	pub remove_from_library: bool,
}

impl CoreAction for DeviceRevokeAction {
	type Output = DeviceRevokeOutput;
	type Input = DeviceRevokeInput;

	fn from_input(input: Self::Input) -> std::result::Result<Self, String> {
		Ok(Self {
			device_id: input.device_id,
			remove_from_library: input.remove_from_library,
		})
	}

	async fn execute(
		self,
		context: Arc<crate::context::CoreContext>,
	) -> std::result::Result<Self::Output, ActionError> {
		tracing::info!("Revoking device: {}", self.device_id);

		let net = context
			.get_networking()
			.await
			.ok_or_else(|| ActionError::Internal("Networking not initialized".to_string()))?;

		// Remove from network registry state and persistence
		{
			let reg = net.device_registry();
			let mut guard = reg.write().await;

			tracing::info!(
				"Removing device {} from network registry in-memory state",
				self.device_id
			);
			if let Err(e) = guard.remove_device(self.device_id) {
				tracing::warn!("Failed to remove device from network registry: {}", e);
			}

			tracing::info!(
				"Removing device {} from network encrypted persistence",
				self.device_id
			);
			match guard.remove_paired_device(self.device_id).await {
				Ok(removed) => {
					if removed {
						tracing::info!(
							"Device {} removed from network persistent storage",
							self.device_id
						);
					} else {
						tracing::warn!(
							"Device {} not found in network persistent storage (already removed?)",
							self.device_id
						);
					}
				}
				Err(e) => {
					tracing::error!("Failed to remove device from network persistence: {}", e);
					return Err(ActionError::Internal(format!(
						"Failed to remove device from network persistence: {}",
						e
					)));
				}
			}
		}

		// Remove from all library databases (if requested)
		if self.remove_from_library {
			tracing::info!(
				"Removing device {} from library databases (remove_from_library=true)",
				self.device_id
			);
			let libraries = context.libraries().await;
			let mut removed_from_libraries = 0;

			for library in libraries.get_open_libraries().await {
				let db = library.db().conn();

				// Delete device from library database
				use crate::infra::db::entities::device;
				use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

				match device::Entity::delete_many()
					.filter(device::Column::Uuid.eq(self.device_id))
					.exec(db)
					.await
				{
					Ok(result) => {
						if result.rows_affected > 0 {
							tracing::info!(
								"Device {} removed from library {} database",
								self.device_id,
								library.id()
							);
							removed_from_libraries += 1;
						}
					}
					Err(e) => {
						tracing::warn!(
							"Failed to remove device from library {} database: {}",
							library.id(),
							e
						);
					}
				}
			}

			if removed_from_libraries > 0 {
				tracing::info!(
					"Device {} removed from {} library database(s)",
					self.device_id,
					removed_from_libraries
				);
			} else {
				tracing::warn!(
					"Device {} not found in any library databases (may have been removed already)",
					self.device_id
				);
			}
		} else {
			tracing::info!(
				"Skipping library database removal for device {} (remove_from_library=false)",
				self.device_id
			);
		}

		// Remove from DeviceManager cache
		tracing::info!(
			"Removing device {} from DeviceManager cache",
			self.device_id
		);
		if let Err(e) = context
			.device_manager
			.remove_paired_device_from_cache(self.device_id)
		{
			tracing::warn!("Failed to remove device from cache: {}", e);
		}

		// Emit ResourceDeleted event
		tracing::info!(
			"Emitting ResourceDeleted event for device {}",
			self.device_id
		);
		use crate::domain::resource::EventEmitter;
		crate::domain::device::Device::emit_deleted(self.device_id, &context.events);

		tracing::info!("Device {} successfully revoked", self.device_id);
		Ok(DeviceRevokeOutput { revoked: true })
	}

	fn action_kind(&self) -> &'static str {
		"network.device.revoke"
	}
}

crate::register_core_action!(DeviceRevokeAction, "network.device.revoke");
