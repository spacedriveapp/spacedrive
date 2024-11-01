use super::error::VolumeError;
use super::types::VolumeEvent;
use crate::volume::{DiskType, FileSystem, MountType};
use std::{collections::HashSet, path::PathBuf, sync::Arc, time::Duration};
use tokio::{
	sync::{broadcast, mpsc, RwLock},
	time::{sleep, Instant},
};
use tracing::error;

const DEBOUNCE_MS: u64 = 100;

/// State of a volume watcher
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
		let (platform_tx, mut platform_rx) = mpsc::unbounded_channel();

		// Start platform-specific watcher
		self.spawn_platform_watcher(platform_tx).await?;

		let event_tx = self.event_tx.clone();
		let ignored_paths = self.ignored_paths.clone();
		let running = self.running.clone();

		// Event processor
		tokio::spawn(async move {
			let mut last_event: Option<VolumeEvent> = None;

			while let Some(event) = platform_rx.recv().await {
				if !*running.read().await {
					break;
				}

				// Skip ignored paths
				if let Some(path) = event.path() {
					if ignored_paths.read().await.contains(path) {
						continue;
					}
				}

				// Simple debouncing
				if let Some(last) = &last_event {
					if last.matches(&event) {
						sleep(Duration::from_millis(DEBOUNCE_MS)).await;
						continue;
					}
				}

				if let Err(e) = event_tx.send(event.clone()) {
					error!("Failed to send volume event: {}", e);
				}

				last_event = Some(event);
			}
		});

		Ok(())
	}

	pub async fn stop(&self) {
		*self.running.write().await = false;
	}

	pub async fn ignore_path(&self, path: PathBuf) {
		self.ignored_paths.write().await.insert(path);
	}

	pub async fn unignore_path(&self, path: &PathBuf) {
		self.ignored_paths.write().await.remove(path);
	}

	async fn spawn_platform_watcher(
		&self,
		tx: mpsc::UnboundedSender<VolumeEvent>,
	) -> Result<(), VolumeError> {
		#[cfg(target_os = "linux")]
		return self.spawn_linux_watcher(tx).await;

		#[cfg(target_os = "macos")]
		return self.spawn_macos_watcher(tx).await;

		#[cfg(target_os = "windows")]
		return self.spawn_windows_watcher(tx).await;
	}

	#[cfg(target_os = "linux")]
	async fn spawn_linux_watcher(
		&self,
		tx: mpsc::UnboundedSender<VolumeEvent>,
	) -> Result<(), VolumeError> {
		use inotify::{Inotify, WatchMask};
		use tokio::io::AsyncReadExt;

		let mut inotify = Inotify::init()
			.map_err(|e| VolumeError::Watcher(format!("Failed to initialize inotify: {}", e)))?;

		// Watch mount points
		for path in ["/dev", "/media", "/mnt"] {
			inotify
				.add_watch(path, WatchMask::CREATE | WatchMask::DELETE)
				.map_err(|e| {
					VolumeError::Watcher(format!("Failed to add watch on {}: {}", path, e))
				})?;
		}

		let running = self.running.clone();

		tokio::spawn(async move {
			let mut buffer = [0; 1024];
			while *running.read().await {
				match inotify.read_events_blocking(&mut buffer) {
					Ok(events) => {
						for event in events {
							let event_type = if event.mask.contains(inotify::EventMask::CREATE) {
								VolumeEvent::VolumeAdded
							} else {
								VolumeEvent::VolumeRemoved
							};
							let _ = tx.send(event_type);
						}
					}
					Err(e) => error!("Inotify error: {}", e),
				}
			}
		});

		Ok(())
	}
	#[cfg(target_os = "macos")]
	async fn spawn_macos_watcher(
		&self,
		tx: mpsc::UnboundedSender<VolumeEvent>,
	) -> Result<(), VolumeError> {
		use fsevent::{self, StreamFlags};

		use crate::volume::Volume;

		// Create channels for fsevent
		let (fs_event_tx, fs_event_rx) = std::sync::mpsc::channel();

		// Spawn thread for fsevent
		std::thread::spawn(move || {
			let mut stream = fsevent::FsEvent::new(vec!["/Volumes".to_string()]);

			stream.observe_async(fs_event_tx).unwrap();
			std::thread::sleep(std::time::Duration::from_secs(5));
			stream.shutdown_observe();
		});

		// Spawn task to process events
		let running = self.running.clone();
		tokio::spawn(async move {
			while *running.read().await {
				match fs_event_rx.try_recv() {
					Ok(event) => {
						// Get current volumes after event
						let volumes_result = super::os::get_volumes().await;

						match volumes_result {
							Ok(current_volumes) => {
								if event.flag.contains(StreamFlags::MOUNT) {
									// Small delay to let the OS finish mounting
									tokio::time::sleep(Duration::from_millis(500)).await;

									// Find newly mounted volume
									if let Some(volume) = current_volumes.iter().find(|v| {
										v.mount_point.to_string_lossy().contains(&event.path)
									}) {
										let _ = tx.send(VolumeEvent::VolumeAdded(volume.clone()));
									}
								} else if event.flag.contains(StreamFlags::UNMOUNT) {
									// For unmount, we need to synthesize a basic volume since it's already gone
									let basic_volume = Volume::new(
										event.path.clone(),
										MountType::External,
										PathBuf::from(&event.path),
										vec![],
										DiskType::Unknown,
										FileSystem::Other("unknown".to_string()),
										0,
										0,
										false,
									);
									let _ = tx.send(VolumeEvent::VolumeRemoved(basic_volume));
								}
							}
							Err(e) => {
								error!("Failed to get volumes after event: {}", e);
								let _ = tx.send(VolumeEvent::VolumeError {
									id: vec![],
									error: format!("Failed to get volumes: {}", e),
								});
							}
						}
					}
					Err(std::sync::mpsc::TryRecvError::Empty) => {
						tokio::time::sleep(Duration::from_millis(100)).await;
					}
					Err(std::sync::mpsc::TryRecvError::Disconnected) => {
						error!("FSEvent channel disconnected");
						break;
					}
				}
			}
		});

		Ok(())
	}

	#[cfg(target_os = "windows")]
	async fn spawn_windows_watcher(
		&self,
		tx: mpsc::UnboundedSender<VolumeEvent>,
	) -> Result<(), VolumeError> {
		use windows::Win32::Storage::FileSystem::{
			ReadDirectoryChangesW, FILE_NOTIFY_CHANGE_DIR_NAME,
		};

		let path = std::ffi::OsString::from("C:\\")
			.encode_wide()
			.chain(std::iter::once(0))
			.collect::<Vec<_>>();

		unsafe {
			let handle = windows::Win32::Storage::FileSystem::CreateFileW(
				path.as_ptr(),
				windows::Win32::Storage::FileSystem::FILE_LIST_DIRECTORY,
				windows::Win32::Storage::FileSystem::FILE_SHARE_READ
					| windows::Win32::Storage::FileSystem::FILE_SHARE_WRITE
					| windows::Win32::Storage::FileSystem::FILE_SHARE_DELETE,
				std::ptr::null_mut(),
				windows::Win32::Storage::FileSystem::OPEN_EXISTING,
				windows::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS
					| windows::Win32::Storage::FileSystem::FILE_FLAG_OVERLAPPED,
				std::ptr::null_mut(),
			);

			if handle.is_invalid() {
				return Err(VolumeError::Watcher(
					"Failed to open directory for watching".into(),
				));
			}

			let running = self.running.clone();

			tokio::spawn(async move {
				let mut buffer = [0u8; 1024];

				while *running.read().await {
					match ReadDirectoryChangesW(
						handle,
						buffer.as_mut_ptr() as *mut _,
						buffer.len() as u32,
						true,
						FILE_NOTIFY_CHANGE_DIR_NAME as u32,
						std::ptr::null_mut(),
						std::ptr::null_mut(),
						None,
					) {
						Ok(_) => {
							let _ = tx.send(VolumeEvent::VolumeAdded);
						}
						Err(e) => error!("ReadDirectoryChangesW error: {}", e),
					}
				}
			});
		}

		Ok(())
	}
}

impl VolumeEvent {
	fn matches(&self, other: &VolumeEvent) -> bool {
		std::mem::discriminant(self) == std::mem::discriminant(other)
	}

	fn path(&self) -> Option<&PathBuf> {
		match self {
			VolumeEvent::VolumeAdded(vol) | VolumeEvent::VolumeRemoved(vol) => {
				Some(&vol.mount_point)
			}
			VolumeEvent::VolumeUpdated { new, .. } => Some(&new.mount_point),
			_ => None,
		}
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

		// Wait for potential events
		let result = timeout(Duration::from_secs(1), rx.recv()).await;

		// Cleanup
		watcher.stop().await;

		if let Ok(Ok(event)) = result {
			println!("Received event: {:?}", event);
		}
	}
}
