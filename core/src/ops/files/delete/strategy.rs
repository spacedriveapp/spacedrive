//! Delete strategy implementations for different deletion scenarios
//!
//! This module implements 2 distinct delete strategies:
//! 1. **LocalDeleteStrategy** - Local file deletion (trash, permanent, secure)
//! 2. **RemoteDeleteStrategy** - Cross-device deletion via network

use crate::{domain::addressing::SdPath, infra::job::prelude::*};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

use super::job::DeleteMode;

/// Result of a delete operation for a single path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResult {
	pub path: SdPath,
	pub success: bool,
	pub bytes_freed: u64,
	pub error: Option<String>,
}

/// Strategy for executing delete operations
#[async_trait]
pub trait DeleteStrategy: Send + Sync {
	/// Execute deletion of paths
	async fn execute(
		&self,
		ctx: &JobContext<'_>,
		paths: &[SdPath],
		mode: DeleteMode,
	) -> Result<Vec<DeleteResult>>;
}

/// Local deletion strategy for same-device operations
pub struct LocalDeleteStrategy;

#[async_trait]
impl DeleteStrategy for LocalDeleteStrategy {
	async fn execute(
		&self,
		ctx: &JobContext<'_>,
		paths: &[SdPath],
		mode: DeleteMode,
	) -> Result<Vec<DeleteResult>> {
		let mut results = Vec::new();

		for path in paths {
			let result = match path {
				// Local physical path - use direct filesystem (fast path)
				_ if path.is_local() => {
					let local_path = path
						.as_local_path()
						.ok_or_else(|| anyhow::anyhow!("Path is not local"))?;

					let size = self.get_path_size(local_path).await.unwrap_or(0);

					let deletion_result = match mode {
						DeleteMode::Trash => self.move_to_trash(local_path).await,
						DeleteMode::Permanent => self.permanent_delete(local_path).await,
						DeleteMode::Secure => self.secure_delete(local_path).await,
					};

					DeleteResult {
						path: path.clone(),
						success: deletion_result.is_ok(),
						bytes_freed: if deletion_result.is_ok() { size } else { 0 },
						error: deletion_result.err().map(|e| e.to_string()),
					}
				}

				// Cloud path - use VolumeBackend
				_ if path.is_cloud() => self.delete_cloud_path(ctx, path, mode.clone()).await,

				// Remote physical or content paths not supported
				_ => DeleteResult {
					path: path.clone(),
					success: false,
					bytes_freed: 0,
					error: Some("Path is remote or unsupported".to_string()),
				},
			};

			results.push(result);
		}

		Ok(results)
	}
}

impl LocalDeleteStrategy {
	/// Delete a cloud path using VolumeBackend
	async fn delete_cloud_path(
		&self,
		ctx: &JobContext<'_>,
		path: &SdPath,
		mode: DeleteMode,
	) -> DeleteResult {
		// Only permanent deletion is supported for cloud paths
		if !matches!(mode, DeleteMode::Permanent) {
			return DeleteResult {
				path: path.clone(),
				success: false,
				bytes_freed: 0,
				error: Some(format!(
					"Delete mode {:?} not supported for cloud paths (only Permanent)",
					mode
				)),
			};
		}

		// Get volume manager
		let volume_manager = match ctx.volume_manager() {
			Some(vm) => vm,
			None => {
				return DeleteResult {
					path: path.clone(),
					success: false,
					bytes_freed: 0,
					error: Some("Volume manager not available".to_string()),
				}
			}
		};

		// Extract cloud path components
		let (service, identifier, cloud_path) = match path.as_cloud() {
			Some((s, i, p)) => (s, i, p),
			None => {
				return DeleteResult {
					path: path.clone(),
					success: false,
					bytes_freed: 0,
					error: Some("Path is not a cloud path".to_string()),
				}
			}
		};

		// Get the volume by service and identifier
		let volume = match volume_manager.find_cloud_volume(service, identifier).await {
			Some(v) => v,
			None => {
				return DeleteResult {
					path: path.clone(),
					success: false,
					bytes_freed: 0,
					error: Some(format!(
						"Cloud volume not found: {} ({})",
						service.scheme(),
						identifier
					)),
				}
			}
		};

		// Get backend
		let backend = match volume.backend.as_ref() {
			Some(b) => b,
			None => {
				return DeleteResult {
					path: path.clone(),
					success: false,
					bytes_freed: 0,
					error: Some("Volume backend not available".to_string()),
				}
			}
		};

		// Get size before deletion (optional, for metrics)
		let size = match backend.metadata(Path::new(cloud_path)).await {
			Ok(metadata) => metadata.size,
			Err(_) => 0, // If we can't get metadata, proceed anyway
		};

		// Perform deletion
		match backend.delete(Path::new(cloud_path)).await {
			Ok(_) => DeleteResult {
				path: path.clone(),
				success: true,
				bytes_freed: size,
				error: None,
			},
			Err(e) => DeleteResult {
				path: path.clone(),
				success: false,
				bytes_freed: 0,
				error: Some(format!("Cloud deletion failed: {}", e)),
			},
		}
	}

	/// Get size of a path (file or directory) using iterative approach
	async fn get_path_size(&self, path: &Path) -> Result<u64, std::io::Error> {
		let mut total = 0u64;
		let mut stack = vec![path.to_path_buf()];

		while let Some(current_path) = stack.pop() {
			let metadata = fs::metadata(&current_path).await?;

			if metadata.is_file() {
				total += metadata.len();
			} else if metadata.is_dir() {
				let mut dir = fs::read_dir(&current_path).await?;
				while let Some(entry) = dir.next_entry().await? {
					stack.push(entry.path());
				}
			}
		}

		Ok(total)
	}

	/// Move file to system trash/recycle bin
	pub async fn move_to_trash(&self, path: &Path) -> Result<(), std::io::Error> {
		#[cfg(target_os = "macos")]
		{
			self.move_to_trash_macos(path).await?;
		}

		#[cfg(all(unix, not(target_os = "macos")))]
		{
			self.move_to_trash_unix(path).await?;
		}

		#[cfg(windows)]
		{
			self.move_to_trash_windows(path).await?;
		}

		Ok(())
	}

	#[cfg(unix)]
	async fn move_to_trash_unix(&self, path: &Path) -> Result<(), std::io::Error> {
		let home = std::env::var("HOME")
			.map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "HOME not set"))?;

		let trash_dir = std::path::Path::new(&home).join(".local/share/Trash/files");
		fs::create_dir_all(&trash_dir).await?;

		let filename = path.file_name().ok_or_else(|| {
			std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid filename")
		})?;

		let trash_path = trash_dir.join(filename);
		let final_trash_path = self.find_unique_trash_name(&trash_path).await?;

		fs::rename(path, final_trash_path).await?;

		Ok(())
	}

	#[cfg(windows)]
	async fn move_to_trash_windows(&self, path: &Path) -> Result<(), std::io::Error> {
		let temp_dir = std::env::temp_dir().join("spacedrive_trash");
		fs::create_dir_all(&temp_dir).await?;

		let filename = path.file_name().ok_or_else(|| {
			std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid filename")
		})?;

		let trash_path = temp_dir.join(filename);
		let final_trash_path = self.find_unique_trash_name(&trash_path).await?;

		fs::rename(path, final_trash_path).await?;

		Ok(())
	}

	#[cfg(target_os = "macos")]
	async fn move_to_trash_macos(&self, path: &Path) -> Result<(), std::io::Error> {
		let home = std::env::var("HOME")
			.map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "HOME not set"))?;

		let trash_dir = std::path::Path::new(&home).join(".Trash");

		let filename = path.file_name().ok_or_else(|| {
			std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid filename")
		})?;

		let trash_path = trash_dir.join(filename);
		let final_trash_path = self.find_unique_trash_name(&trash_path).await?;

		fs::rename(path, final_trash_path).await?;

		Ok(())
	}

	/// Find a unique name in the trash directory
	async fn find_unique_trash_name(&self, base_path: &Path) -> Result<PathBuf, std::io::Error> {
		let mut candidate = base_path.to_path_buf();
		let mut counter = 1;

		while fs::try_exists(&candidate).await? {
			let stem = base_path.file_stem().unwrap_or_default();
			let extension = base_path.extension();

			let new_name = if let Some(ext) = extension {
				format!("{} ({})", stem.to_string_lossy(), counter)
			} else {
				format!("{} ({})", stem.to_string_lossy(), counter)
			};

			candidate = base_path.with_file_name(new_name);
			if let Some(ext) = extension {
				candidate.set_extension(ext);
			}

			counter += 1;
		}

		Ok(candidate)
	}

	/// Permanently delete file or directory
	pub async fn permanent_delete(&self, path: &Path) -> Result<(), std::io::Error> {
		let metadata = fs::metadata(path).await?;

		if metadata.is_file() {
			fs::remove_file(path).await?;
		} else if metadata.is_dir() {
			fs::remove_dir_all(path).await?;
		}

		Ok(())
	}

	/// Securely delete file by overwriting with random data
	pub async fn secure_delete(&self, path: &Path) -> Result<(), std::io::Error> {
		let metadata = fs::metadata(path).await?;

		if metadata.is_file() {
			self.secure_overwrite_file(path, metadata.len()).await?;
			fs::remove_file(path).await?;
		} else if metadata.is_dir() {
			self.secure_delete_directory(path).await?;
			fs::remove_dir_all(path).await?;
		}

		Ok(())
	}

	/// Securely overwrite a file with random data
	async fn secure_overwrite_file(&self, path: &Path, size: u64) -> Result<(), std::io::Error> {
		use rand::RngCore;
		use tokio::io::{AsyncSeekExt, AsyncWriteExt};

		let mut file = fs::OpenOptions::new()
			.write(true)
			.truncate(false)
			.open(path)
			.await?;

		// Overwrite with random data (3 passes)
		for _ in 0..3 {
			file.seek(std::io::SeekFrom::Start(0)).await?;

			let mut remaining = size;

			while remaining > 0 {
				let chunk_size = std::cmp::min(remaining, 64 * 1024) as usize;

				let buffer = {
					let mut rng = rand::thread_rng();
					let mut buf = vec![0u8; chunk_size];
					rng.fill_bytes(&mut buf);
					buf
				};

				file.write_all(&buffer).await?;
				remaining -= chunk_size as u64;
			}

			file.flush().await?;
			file.sync_all().await?;
		}

		Ok(())
	}

	/// Secure delete directory using iterative approach
	async fn secure_delete_directory(&self, path: &Path) -> Result<(), std::io::Error> {
		let mut stack = vec![path.to_path_buf()];

		while let Some(current_path) = stack.pop() {
			let mut dir = fs::read_dir(&current_path).await?;

			while let Some(entry) = dir.next_entry().await? {
				let entry_path = entry.path();

				if entry_path.is_file() {
					let metadata = fs::metadata(&entry_path).await?;
					self.secure_overwrite_file(&entry_path, metadata.len())
						.await?;
					fs::remove_file(&entry_path).await?;
				} else if entry_path.is_dir() {
					stack.push(entry_path);
				}
			}
		}

		Ok(())
	}
}

/// Remote deletion strategy for cross-device operations
pub struct RemoteDeleteStrategy;

#[async_trait]
impl DeleteStrategy for RemoteDeleteStrategy {
	async fn execute(
		&self,
		ctx: &JobContext<'_>,
		paths: &[SdPath],
		mode: DeleteMode,
	) -> Result<Vec<DeleteResult>> {
		// Group paths by target device
		let mut by_device: HashMap<Uuid, Vec<SdPath>> = HashMap::new();
		for path in paths {
			if let Some(device_id) = path.device_id() {
				by_device.entry(device_id).or_default().push(path.clone());
			}
		}

		let mut all_results = Vec::new();

		// Send delete request to each device
		for (device_id, device_paths) in by_device {
			let results = self
				.delete_on_device(ctx, device_id, &device_paths, mode.clone())
				.await?;
			all_results.extend(results);
		}

		Ok(all_results)
	}
}

impl RemoteDeleteStrategy {
	async fn delete_on_device(
		&self,
		ctx: &JobContext<'_>,
		device_id: Uuid,
		paths: &[SdPath],
		mode: DeleteMode,
	) -> Result<Vec<DeleteResult>> {
		let networking = ctx
			.networking_service()
			.ok_or_else(|| anyhow::anyhow!("Networking service not available"))?;

		let request_id = Uuid::new_v4();

		// Create delete request
		let request = FileDeleteMessage::Request {
			paths: paths.to_vec(),
			mode,
			request_id,
		};

		// Serialize request
		let request_data = rmp_serde::to_vec(&request)?;

		ctx.log(format!(
			"Sending delete request to device {} for {} paths",
			device_id,
			paths.len()
		));

		// Send request via networking service
		let networking_guard = &*networking;
		networking_guard
			.send_message(device_id, "file_delete", request_data)
			.await?;

		// TODO: Implement proper request/response pattern
		// For now, return optimistic results
		// In production, we need to wait for response from remote device
		let results = paths
			.iter()
			.map(|path| DeleteResult {
				path: path.clone(),
				success: true,
				bytes_freed: 0,
				error: None,
			})
			.collect();

		ctx.log(format!(
			"Delete request sent to device {}, {} paths",
			device_id,
			paths.len()
		));

		Ok(results)
	}
}

/// Network protocol message for remote deletion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileDeleteMessage {
	Request {
		paths: Vec<SdPath>,
		mode: DeleteMode,
		request_id: Uuid,
	},
	Response {
		request_id: Uuid,
		results: Vec<DeleteResult>,
	},
}
