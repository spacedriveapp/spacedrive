//! Volume encryption query
//!
//! Query encryption status for one or more paths. Used by the frontend to
//! determine optimal secure delete strategies before initiating deletion.

use super::output::{PathEncryptionInfo, VolumeEncryptionOutput};
use crate::{
	context::CoreContext,
	domain::volume::DiskType,
	infra::query::{CoreQuery, QueryResult},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{path::Path, sync::Arc};

/// Input for volume encryption query
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeEncryptionQueryInput {
	/// Paths to check encryption status for
	pub paths: Vec<String>,
}

/// Query for checking encryption status of volumes containing specific paths
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeEncryptionQuery {
	paths: Vec<String>,
}

impl CoreQuery for VolumeEncryptionQuery {
	type Input = VolumeEncryptionQueryInput;
	type Output = VolumeEncryptionOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { paths: input.paths })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let volume_manager = &context.volume_manager;
		let mut path_infos = Vec::with_capacity(self.paths.len());

		for path_str in &self.paths {
			let path = Path::new(path_str);

			// Try to find the volume containing this path
			let volume = volume_manager.volume_for_path(path).await;

			let info = match volume {
				Some(vol) => {
					let is_encrypted = vol.is_encrypted();
					let encryption_type = vol.encryption_type().map(|e| e.to_string());
					let is_unlocked = vol.encryption.as_ref().map(|e| e.is_unlocked);
					let recommended_passes = vol.recommended_secure_delete_passes() as u32;

					// Determine if TRIM should be used
					let is_ssd = matches!(vol.disk_type, DiskType::SSD);

					let recommendation_reason = if is_encrypted {
						format!(
							"Volume is encrypted with {}. Single pass sufficient as data is ciphertext.",
							encryption_type.as_deref().unwrap_or("unknown encryption")
						)
					} else if is_ssd {
						"Unencrypted SSD. Single pass with TRIM recommended for wear leveling."
							.to_string()
					} else {
						"Unencrypted HDD. Multiple passes recommended for magnetic remnants."
							.to_string()
					};

					PathEncryptionInfo {
						path: path_str.clone(),
						is_encrypted,
						encryption_type,
						is_unlocked,
						recommended_passes,
						use_trim: is_ssd,
						volume_fingerprint: Some(vol.fingerprint.0.clone()),
						volume_id: Some(vol.id),
						recommendation_reason,
					}
				}
				None => {
					// Volume not found - use conservative defaults
					PathEncryptionInfo {
						path: path_str.clone(),
						is_encrypted: false,
						encryption_type: None,
						is_unlocked: None,
						recommended_passes: 3,
						use_trim: false,
						volume_fingerprint: None,
						volume_id: None,
						recommendation_reason:
							"Volume not detected. Using conservative 3-pass default.".to_string(),
					}
				}
			};

			path_infos.push(info);
		}

		Ok(VolumeEncryptionOutput { paths: path_infos })
	}
}

crate::register_core_query!(VolumeEncryptionQuery, "volumes.encryption");
