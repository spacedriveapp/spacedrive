use super::error::VolumeError;
use super::types::VolumeEvent;
use std::{collections::HashSet, path::PathBuf, sync::Arc, time::Duration};
use tokio::{
	sync::{broadcast, mpsc, RwLock},
	time::{sleep, Instant},
};
use tracing::{debug, error, warn};

const DEBOUNCE_MS: u64 = 100;

#[derive(Debug)]
pub struct WatcherState {
	pub watcher: Arc<VolumeWatcher>,
	pub last_event: Instant,
	pub paused: bool,
}

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

	pub async fn start(&self) -> Result<(), VolumeError> {
		debug!("Starting volume watcher");

		let (check_tx, mut check_rx) = mpsc::channel(1);

		// Start OS-specific watcher
		self.spawn_platform_watcher(check_tx.clone()).await?;

		// Handle volume checks when triggered by OS events
		let event_tx = self.event_tx.clone();
		let running = self.running.clone();

		tokio::spawn(async move {
			let mut last_check = Instant::now();
			let mut last_volumes = Vec::new();

			while *running.read().await {
				// Wait for check trigger from OS watcher
				if check_rx.recv().await.is_some() {
					// Debounce checks
					if last_check.elapsed() < Duration::from_millis(DEBOUNCE_MS) {
						continue;
					}
					last_check = Instant::now();

					match super::os::get_volumes().await {
						Ok(current_volumes) => {
							// Find new volumes
							for volume in &current_volumes {
								if !last_volumes.iter().any(|v| v == volume) {
									debug!("New volume detected: {}", volume.name);
									let _ = event_tx.send(VolumeEvent::VolumeAdded(volume.clone()));
								}
							}

							// Find removed volumes
							for volume in &last_volumes {
								if !current_volumes.iter().any(|v| v == volume) {
									debug!("Volume removed: {}", volume.name);
									let _ =
										event_tx.send(VolumeEvent::VolumeRemoved(volume.clone()));
								}
							}

							// Find updated volumes
							for old_volume in &last_volumes {
								if let Some(new_volume) =
									current_volumes.iter().find(|v| *v == old_volume)
								{
									if new_volume != old_volume {
										debug!("Volume updated: {}", new_volume.name);
										let _ = event_tx.send(VolumeEvent::VolumeUpdated {
											old: old_volume.clone(),
											new: new_volume.clone(),
										});
									}
								}
							}

							last_volumes = current_volumes;
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

			// Watch for volume mount events
			std::thread::spawn(move || {
				let mut stream = fsevent::FsEvent::new(vec![
					"/Volumes".to_string(),
					"/System/Volumes".to_string(),
				]);

				stream
					.observe_async(fs_tx)
					.expect("Failed to start FSEvent stream");
			});

			tokio::spawn(async move {
				while *running.read().await {
					if let Ok(events) = fs_rx.try_recv() {
						if events.flag.contains(StreamFlags::MOUNT)
							|| events.flag.contains(StreamFlags::UNMOUNT)
						{
							if let Err(e) = check_tx.send(()).await {
								error!("Failed to trigger volume check: {}", e);
							}
						}
					}
					sleep(Duration::from_millis(100)).await;
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

#[cfg(test)]
mod tests {
	use super::*;
	use tokio::time::timeout;

	#[tokio::test]
	async fn test_watcher() {
		let (tx, mut rx) = broadcast::channel(16);
		let watcher = VolumeWatcher::new(tx);

		watcher.start().await.expect("Failed to start watcher");

		// Wait for potential volume events
		let result = timeout(Duration::from_secs(2), rx.recv()).await;

		// Cleanup
		watcher.stop().await;

		if let Ok(Ok(event)) = result {
			println!("Received volume event: {:?}", event);
		}
	}
}
