use sd_prisma::prisma::{volume, PrismaClient};

use super::{os::get_volumes, MountType, Volume};
use crate::volume::{speed::SpeedTest, watcher::VolumeWatcher};
use std::sync::Arc;
use tracing::{error, info};
pub struct VolumeManager {
	library_volumes: Vec<Volume>,   // Volumes committed to the DB
	untracked_volumes: Vec<Volume>, // Uncommitted volumes
}

// The volume manager must be conservative with when to trigger a sync event.
// It should only trigger a sync event when a volume is added or removed from the library.
// not when a volume is mounted or unmounted.

impl VolumeManager {
	/// Initializes the VolumeManager by detecting system, external, and network volumes
	pub async fn new(db: &PrismaClient) -> Self {
		// Detect volumes present in the system
		let detected_volumes = VolumeManager::detect_system_volumes().await;

		// Query database for volumes already committed to the library
		let library_volumes = VolumeManager::query_library_volumes(db).await;

		// Merge detected volumes with library volumes to find untracked volumes
		let untracked_volumes =
			VolumeManager::derive_untracked_volumes(&detected_volumes, &library_volumes);

		// We always track system volumes
		VolumeManager::init_system_volumes(db, &untracked_volumes).await;

		info!(?untracked_volumes, "Untracked volumes");

		VolumeManager {
			library_volumes,
			untracked_volumes,
		}
	}

	/// Generates a unique fingerprint for a volume
	pub fn generate_fingerprint(volume: &Volume) -> String {
		// Attempt to read `.spacedrive` fingerprint file if present
		if let Some(fingerprint) = VolumeManager::read_spacedrive_file(&volume.mount_points) {
			return fingerprint;
		}

		// Generate composite fingerprint
		let mut hasher = Sha256::new();
		hasher.update(volume.name.as_bytes());
		hasher.update(volume.file_system.to_string().as_bytes());
		hasher.update(volume.total_bytes_capacity.to_be_bytes());

		let uuid_str = volume
			.pub_id
			.as_ref()
			.map_or("".to_string(), |id| hex::encode(id));
		hasher.update(uuid_str.as_bytes());

		let fingerprint = format!("{:x}", hasher.finalize());
		info!("Generated fingerprint for volume: {}", fingerprint);
		fingerprint
	}

	/// Reads the `.spacedrive` file from the volume if available.
	fn read_spacedrive_file(mount_points: &[PathBuf]) -> Option<String> {
		for mount in mount_points {
			let spacedrive_file = mount.join(".sdvol");
			if let Ok(content) = fs::read_to_string(&spacedrive_file) {
				info!("Found .spacedrive fingerprint: {}", content);
				return Some(content.trim().to_string());
			}
		}
		None
	}

	/// Writes a `.spacedrive` file to the volume for persistent fingerprinting.
	pub fn write_spacedrive_file(volume: &Volume, fingerprint: &str) -> Result<(), VolumeError> {
		if let Some(mount) = volume.mount_points.first() {
			let spacedrive_file = mount.join(".sdvol");
			fs::write(&spacedrive_file, fingerprint).map_err(|e| {
				error!(?e, "Failed to write .spacedrive file");
				VolumeError::DirectoryError(e.to_string())
			})?;
			info!("Written .spacedrive file to {:?}", spacedrive_file);
		}
		Ok(())
	}

	pub async fn start(db: &PrismaClient) -> Arc<Self> {
		// start the volume manager
		let manager = Arc::new(VolumeManager::new(db).await);
		// and the watcher
		spawn_volume_watcher(manager.clone());
		return manager;
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

	// pub fn get_volume_for_location(&self, location: &Location) -> Option<Volume>

	pub async fn track_volume(&mut self, db: &PrismaClient, volume_pub_id: Vec<u8>) {
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
				.retain(|vol| vol.mount_points != volume.mount_points);
			volume.create(&db).await.unwrap_or(());
			self.library_volumes.push(volume.clone());
			info!("Volume tracked: {:?}", volume);

			// spawn a task to test the speed of the volume
			tokio::spawn(async move {
				let mut volume = volume;
				let speed = volume.speed_test().await.unwrap_or((0.0, 0.0));
				info!("Volume speed test: {:?}", speed);
			});
		}
	}

	// triggered by the watcher when a volume is added or removed
	pub async fn evaluate_system_volumes(&self) {
		let volumes = get_volumes().await;
		// get the current volumes on the system
		let detected_volumes = VolumeManager::detect_system_volumes().await;
	}

	pub async fn get_local_volumes(db: &PrismaClient) {}

	// this function will commit the system volumes to the database
	async fn init_system_volumes(db: &PrismaClient, untracked_volumes: &Vec<Volume>) {
		// for each volume, if system volume, commit to db
		for vol in untracked_volumes {
			if vol.mount_type == MountType::System {
				let mut volume_clone = vol.clone();
				// run speed test but fail silently
				volume_clone.speed_test().await.unwrap_or((0.0, 0.0));

				volume_clone.create(db).await.unwrap_or(());
			}
		}
	}

	// async fn register_non_system_volumes(db: &PrismaClient, volumes: &Vec<Volume>) {

	// }
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
					.any(|lib_vol| lib_vol.mount_points == detected_vol.mount_points)
			})
			.cloned()
			.collect()
	}
}
