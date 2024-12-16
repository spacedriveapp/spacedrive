use crate::volume::types::VolumeFingerprint;

use super::error::VolumeError;
use super::types::VolumeEvent;
use super::VolumeManagerActor;
use sd_core_sync::DevicePubId;
use std::{collections::HashSet, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tokio::{
	sync::{broadcast, mpsc, RwLock},
	time::{sleep, Instant},
};
use tracing::{debug, error, warn};

const DEBOUNCE_MS: u64 = 100;

#[derive(Debug)]
pub struct VolumeWatcher {
	event_tx: broadcast::Sender<VolumeEvent>,
	ignored_paths: Arc<RwLock<HashSet<PathBuf>>>,
	running: Arc<RwLock<bool>>,
}

impl VolumeWatcher {
	pub fn new(event_tx: broadcast::Sender<VolumeEvent>) -> Self {
		Self {
			event_tx,
			ignored_paths: Arc::new(RwLock::new(HashSet::new())),
			running: Arc::new(RwLock::new(true)),
		}
	}

	pub async fn start(
		&self,
		device_id: DevicePubId,
		actor: Arc<Mutex<VolumeManagerActor>>,
	) -> Result<(), VolumeError> {
		debug!("Starting volume watcher");

		let (check_tx, mut check_rx) = mpsc::channel(1);

		// Start OS-specific watcher
		self.spawn_platform_watcher(check_tx.clone()).await?;

		// Handle volume checks when triggered by OS events
		let event_tx = self.event_tx.clone();
		let running = self.running.clone();

		tokio::spawn(async move {
			let mut last_check = Instant::now();

			while *running.read().await {
				// Wait for check trigger from OS watcher
				if check_rx.recv().await.is_some() {
					// Debounce checks
					if last_check.elapsed() < Duration::from_millis(DEBOUNCE_MS) {
						continue;
					}
					last_check = Instant::now();

					match super::os::get_volumes().await {
						Ok(discovered_volumes) => {
							let actor = actor.lock().await;

							// Find new volumes
							for volume in &discovered_volumes {
								let fingerprint = VolumeFingerprint::new(&device_id, volume);

								let volume_exists = actor.volume_exists(fingerprint.clone()).await;
								// if the volume doesn't exist in the actor state, we need to send an event
								if !volume_exists {
									let _ = event_tx.send(VolumeEvent::VolumeAdded(volume.clone()));
								}
							}

							// Find removed volumes and send an event
							for volume in &actor.get_volumes().await {
								let fingerprint = VolumeFingerprint::new(&device_id, volume);
								if !discovered_volumes
									.iter()
									.any(|v| VolumeFingerprint::new(&device_id, v) == fingerprint)
								{
									let _ =
										event_tx.send(VolumeEvent::VolumeRemoved(volume.clone()));
								}
							}
						}
						Err(e) => {
							warn!("Failed to get volumes during watch: {}", e);
						}
					}
				}
			}
		});

		Ok(())
	}

	async fn spawn_platform_watcher(&self, check_tx: mpsc::Sender<()>) -> Result<(), VolumeError> {
		let running = self.running.clone();

		#[cfg(target_os = "linux")]
		{
			use inotify::{Inotify, WatchMask};

			let inotify = Inotify::init().map_err(|e| {
				VolumeError::Platform(format!("Failed to initialize inotify: {}", e))
			})?;

			// Watch mount points and device changes
			for path in ["/dev", "/media", "/mnt", "/run/media"] {
				if let Err(e) = inotify.add_watch(
					path,
					WatchMask::CREATE | WatchMask::DELETE | WatchMask::MODIFY | WatchMask::UNMOUNT,
				) {
					warn!("Failed to watch path {}: {}", path, e);
				}
			}

			let check_tx = check_tx.clone();
			tokio::spawn(async move {
				let mut buffer = [0; 4096];
				while *running.read().await {
					match inotify.read_events_blocking(&mut buffer) {
						Ok(_) => {
							if let Err(e) = check_tx.send(()).await {
								error!("Failed to trigger volume check: {}", e);
							}
						}
						Err(e) => error!("Inotify error: {}", e),
					}
				}
			});
		}

		#[cfg(target_os = "macos")]
		{
			use fsevent::{self, StreamFlags};

			let (fs_tx, fs_rx) = std::sync::mpsc::channel();
			let check_tx = check_tx.clone();

			// Keep stream alive in the thread
			std::thread::spawn(move || {
				let mut stream = fsevent::FsEvent::new(vec![
					"/Volumes".to_string(),
					"/System/Volumes".to_string(),
				]);

				match stream.observe_async(fs_tx) {
					Ok(_) => {
						// Block thread to keep stream alive
						std::thread::park();
					}
					Err(e) => {
						error!("Failed to start FSEvent stream: {}", e);
					}
				}
			});

			tokio::spawn(async move {
				while *running.read().await {
					match fs_rx.recv() {
						Ok(events) => {
							// Only care about mount/unmount events
							if events.flag.contains(StreamFlags::MOUNT)
								|| events.flag.contains(StreamFlags::UNMOUNT)
							{
								debug!("Received volume event: {:?}", events);
								if let Err(e) = check_tx.send(()).await {
									error!("Failed to trigger volume check: {}", e);
								}
							}
						}
						Err(e) => {
							error!("FSEvent receive error: {}", e);
							sleep(Duration::from_millis(100)).await;
						}
					}
				}
			});
		}

		#[cfg(target_os = "windows")]
		{
			use windows::Win32::Storage::FileSystem::{
				FindFirstVolumeW, FindNextVolumeW, FindVolumeClose, ReadDirectoryChangesW,
				FILE_NOTIFY_CHANGE_DIR_NAME,
			};

			let check_tx = check_tx.clone();
			tokio::spawn(async move {
				while *running.read().await {
					// Watch for volume arrival/removal
					unsafe {
						let mut volume_name = [0u16; 260];
						let handle = FindFirstVolumeW(volume_name.as_mut_ptr());
						if !handle.is_invalid() {
							// Volume change detected
							if let Err(e) = check_tx.send(()).await {
								error!("Failed to trigger volume check: {}", e);
							}
							FindVolumeClose(handle);
						}
					}
					sleep(Duration::from_millis(100)).await;
				}
			});
		}

		Ok(())
	}

	pub async fn stop(&self) {
		debug!("Stopping volume watcher");
		*self.running.write().await = false;
	}

	pub async fn ignore_path(&self, path: PathBuf) {
		self.ignored_paths.write().await.insert(path);
	}

	pub async fn unignore_path(&self, path: &PathBuf) {
		self.ignored_paths.write().await.remove(path);
	}
}

// #[cfg(test)]
// mod tests {
// 	use super::*;
// 	use tokio::time::timeout;

// 	#[tokio::test]
// 	async fn test_watcher() {
// 		let (tx, mut rx) = broadcast::channel(16);
// 		let watcher = VolumeWatcher::new(tx);

// 		watcher.start().await.expect("Failed to start watcher");

// 		// Wait for potential volume events
// 		let result = timeout(Duration::from_secs(2), rx.recv()).await;

// 		// Cleanup
// 		watcher.stop().await;

// 		if let Ok(Ok(event)) = result {
// 			println!("Received volume event: {:?}", event);
// 		}
// 	}
// }
