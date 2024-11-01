use sd_prisma::prisma::{device, volume, PrismaClient};

use super::{os::get_volumes, MountType, Volume, VolumeError};
use crate::{invalidate_query, library::Library, volume::speed::SpeedTest};
use std::sync::Arc;
use tracing::{error, info};

pub struct VolumeManager {
	library: Arc<Library>,
	current_device_id: i32,         // the db id of the current device
	library_volumes: Vec<Volume>,   // Volumes committed to the DB
	untracked_volumes: Vec<Volume>, // Uncommitted volumes
}

// The volume manager must be conservative with when to trigger a sync event.
// It should only trigger a sync event when a volume is added or removed from the library.
// not when a volume is mounted or unmounted.
impl VolumeManager {
	/// Initializes the VolumeManager by detecting system, external, and network volumes
	pub async fn new(lib: Arc<Library>) -> Result<Self, VolumeError> {
		// Detect volumes present in the system
		let detected_volumes = VolumeManager::detect_system_volumes().await;

		// Query database for volumes already committed to the library
		let library_volumes = VolumeManager::query_library_volumes(&lib.db).await;

		// Merge detected volumes with library volumes to find untracked volumes
		let untracked_volumes =
			VolumeManager::derive_untracked_volumes(&detected_volumes, &library_volumes);

		let current_device = lib
			.db
			.device()
			.find_unique(device::pub_id::equals(lib.sync.device_pub_id.to_db()))
			.select(device::select!({ id }))
			.exec()
			.await?
			.ok_or(VolumeError::NoDeviceFound)?;

		info!("VOLUME MANAGER: Current device: {:?}", current_device);

		Ok(VolumeManager {
			current_device_id: current_device.id,
			library: lib.clone(),
			library_volumes,
			untracked_volumes,
		})
	}

	/// Detects system volumes (external, system, etc.)
	async fn detect_system_volumes() -> Vec<Volume> {
		get_volumes().await
	}

	/// Queries volumes already in the library (database)
	async fn query_library_volumes(db: &PrismaClient) -> Vec<Volume> {
		match db.volume().find_many(vec![]).exec().await {
			Ok(volumes) => volumes.into_iter().map(Volume::from).collect(),
			Err(e) => {
				error!(?e, "Failed to query library volumes;");
				vec![]
			}
		}
	}

	pub async fn track_volume(&mut self, volume_pub_id: Vec<u8>) {
		// if volume is already tracked, do nothing
		if self
			.library_volumes
			.iter()
			.any(|vol| vol.pub_id == Some(volume_pub_id.clone()))
		{
			return;
		}

		let volume_index = self
			.untracked_volumes
			.iter()
			.position(|vol| vol.pub_id == Some(volume_pub_id.clone()));

		if let Some(index) = volume_index {
			// remove the volume from the untracked volumes
			let volume = self.untracked_volumes.swap_remove(index);

			// update the volume in the library
			self.untracked_volumes
				.retain(|vol| vol.mount_point != volume.mount_point);

			volume
				.create(&self.library.db, self.current_device_id)
				.await
				.unwrap_or(());

			self.library_volumes.push(volume.clone());
			info!("Volume tracked: {:?}", volume);

			let _lib = self.library.clone();
			// spawn a task to test the speed of the volume
			tokio::spawn(async move {
				let mut volume = volume;
				let speed = volume.speed_test().await.unwrap_or((0.0, 0.0));
				info!("Volume speed test: {:?}", speed);
			});
		}
	}

	// triggered by the watcher when a volume is added or removed
	pub async fn evaluate_system_volumes(&self) -> Result<(), VolumeError> {
		let volumes = get_volumes().await;
		// get the current volumes on the system
		let detected_volumes = VolumeManager::detect_system_volumes().await;

		println!("Volumes: {:?}", volumes);
		println!("Detected Volumes: {:?}", detected_volumes);

		Ok(())
	}

	// this function will commit the system volumes to the database
	async fn init_system_volumes(&self) -> Result<(), VolumeError> {
		// for each volume, if system volume, commit to db
		for vol in self.untracked_volumes.clone() {
			if vol.mount_type == MountType::System {
				println!("ADDING SYSTEM VOLUME");
				let mut volume_clone = vol.clone();
				// run speed test but fail silently
				volume_clone.speed_test().await.unwrap_or((0.0, 0.0));

				volume_clone
					.create(&self.library.db, self.current_device_id)
					.await
					.unwrap_or(());

				println!("Volume created: {:?}", volume_clone);
			}
		}
		Ok(())
	}

	/// Finds untracked volumes by comparing detected and library volumes
	fn derive_untracked_volumes(
		detected_volumes: &Vec<Volume>,
		library_volumes: &Vec<Volume>,
	) -> Vec<Volume> {
		// Filter out volumes that are already in the library
		detected_volumes
			.iter()
			.filter(|detected_vol| {
				!library_volumes
					.iter()
					.any(|lib_vol| lib_vol.mount_point == detected_vol.mount_point)
			})
			.cloned()
			.collect()
	}

	// async fn register_non_system_volumes(db: &PrismaClient, volumes: &Vec<Volume>) {}
	// pub async fn get_local_volumes(db: &PrismaClient) {}
	// pub fn get_volume_for_location(&self, location: &Location) -> Option<Volume>
}
