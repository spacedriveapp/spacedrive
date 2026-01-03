//! Test snapshot management for preserving test state

use chrono::Utc;
use std::{
	fs,
	path::{Path, PathBuf},
	sync::atomic::{AtomicBool, Ordering},
};

/// Manages test snapshots for post-mortem debugging
pub struct SnapshotManager {
	test_name: String,
	test_data_path: PathBuf,
	snapshot_base_path: PathBuf,
	timestamp: String,
	captured: AtomicBool,
}

impl SnapshotManager {
	/// Create new snapshot manager
	///
	/// Snapshots are stored in platform-appropriate location:
	/// - macOS: ~/Library/Application Support/spacedrive/test_snapshots/
	/// - Linux: ~/.local/share/spacedrive/test_snapshots/
	/// - Windows: %APPDATA%\spacedrive\test_snapshots\
	pub fn new(test_name: &str, test_data_path: &Path) -> anyhow::Result<Self> {
		let snapshot_base = Self::get_snapshot_base_path()?;
		let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();

		Ok(Self {
			test_name: test_name.to_string(),
			test_data_path: test_data_path.to_path_buf(),
			snapshot_base_path: snapshot_base,
			timestamp,
			captured: AtomicBool::new(false),
		})
	}

	/// Get platform-appropriate snapshot base path
	fn get_snapshot_base_path() -> anyhow::Result<PathBuf> {
		let base = if cfg!(target_os = "macos") {
			let home = std::env::var("HOME")?;
			PathBuf::from(home).join("Library/Application Support/spacedrive/test_snapshots")
		} else if cfg!(target_os = "windows") {
			let appdata = std::env::var("APPDATA")?;
			PathBuf::from(appdata).join("spacedrive\\test_snapshots")
		} else {
			// Linux and other Unix-like systems
			let home = std::env::var("HOME")?;
			PathBuf::from(home).join(".local/share/spacedrive/test_snapshots")
		};

		fs::create_dir_all(&base)?;
		Ok(base)
	}

	/// Capture snapshot with optional label (e.g., "after_phase_1")
	pub async fn capture(&self, label: impl Into<String>) -> anyhow::Result<PathBuf> {
		let label = label.into();
		let snapshot_path = self
			.snapshot_base_path
			.join(&self.test_name)
			.join(format!("{}_{}", self.timestamp, label));

		self.capture_to_path(&snapshot_path).await?;
		self.captured.store(true, Ordering::SeqCst);

		Ok(snapshot_path)
	}

	/// Capture final snapshot (called automatically on drop if not already captured)
	pub async fn capture_final(&self) -> anyhow::Result<PathBuf> {
		self.capture("final").await
	}

	/// Capture final snapshot using blocking operations (for use in Drop)
	pub(crate) fn capture_final_blocking(&self) -> anyhow::Result<PathBuf> {
		let snapshot_path = self
			.snapshot_base_path
			.join(&self.test_name)
			.join(format!("{}_final", self.timestamp));

		self.capture_to_path_blocking(&snapshot_path)?;
		self.captured.store(true, Ordering::SeqCst);

		Ok(snapshot_path)
	}

	/// Check if snapshot has been captured
	pub fn captured(&self) -> bool {
		self.captured.load(Ordering::SeqCst)
	}

	/// Get snapshot path for this test run
	pub fn snapshot_path(&self) -> PathBuf {
		self.snapshot_base_path
			.join(&self.test_name)
			.join(&self.timestamp)
	}

	/// Async capture to path
	async fn capture_to_path(&self, snapshot_path: &Path) -> anyhow::Result<()> {
		tokio::fs::create_dir_all(snapshot_path).await?;

		// Copy core_data directory (databases, etc.)
		let core_data_src = self.test_data_path.join("core_data");
		if tokio::fs::try_exists(&core_data_src).await.unwrap_or(false) {
			let core_data_dst = snapshot_path.join("core_data");
			self.copy_dir_async(&core_data_src, &core_data_dst).await?;
		}

		// Copy logs directory
		let logs_src = self.test_data_path.join("logs");
		if tokio::fs::try_exists(&logs_src).await.unwrap_or(false) {
			let logs_dst = snapshot_path.join("logs");
			self.copy_dir_async(&logs_src, &logs_dst).await?;
		}

		// Write summary
		self.write_summary(snapshot_path).await?;

		Ok(())
	}

	/// Blocking capture to path (for use in Drop)
	fn capture_to_path_blocking(&self, snapshot_path: &Path) -> anyhow::Result<()> {
		fs::create_dir_all(snapshot_path)?;

		// Copy core_data directory (databases, etc.)
		let core_data_src = self.test_data_path.join("core_data");
		if core_data_src.exists() {
			let core_data_dst = snapshot_path.join("core_data");
			self.copy_dir_blocking(&core_data_src, &core_data_dst)?;
		}

		// Copy logs directory
		let logs_src = self.test_data_path.join("logs");
		if logs_src.exists() {
			let logs_dst = snapshot_path.join("logs");
			self.copy_dir_blocking(&logs_src, &logs_dst)?;
		}

		// Write summary
		self.write_summary_blocking(snapshot_path)?;

		Ok(())
	}

	/// Recursively copy directory (async)
	fn copy_dir_async<'a>(
		&'a self,
		src: &'a Path,
		dst: &'a Path,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + 'a>> {
		Box::pin(async move {
			tokio::fs::create_dir_all(dst).await?;

			let mut entries = tokio::fs::read_dir(src).await?;
			while let Some(entry) = entries.next_entry().await? {
				let ty = entry.file_type().await?;
				let src_path = entry.path();
				let dst_path = dst.join(entry.file_name());

				if ty.is_dir() {
					self.copy_dir_async(&src_path, &dst_path).await?;
				} else {
					tokio::fs::copy(&src_path, &dst_path).await?;
				}
			}

			Ok(())
		})
	}

	/// Recursively copy directory (blocking)
	fn copy_dir_blocking(&self, src: &Path, dst: &Path) -> anyhow::Result<()> {
		fs::create_dir_all(dst)?;

		for entry in fs::read_dir(src)? {
			let entry = entry?;
			let ty = entry.file_type()?;
			let src_path = entry.path();
			let dst_path = dst.join(entry.file_name());

			if ty.is_dir() {
				self.copy_dir_blocking(&src_path, &dst_path)?;
			} else {
				fs::copy(&src_path, &dst_path)?;
			}
		}

		Ok(())
	}

	/// Write summary markdown (async)
	async fn write_summary(&self, snapshot_path: &Path) -> anyhow::Result<()> {
		let summary = self.generate_summary(snapshot_path)?;
		tokio::fs::write(snapshot_path.join("summary.md"), summary).await?;
		Ok(())
	}

	/// Write summary markdown (blocking)
	fn write_summary_blocking(&self, snapshot_path: &Path) -> anyhow::Result<()> {
		let summary = self.generate_summary(snapshot_path)?;
		fs::write(snapshot_path.join("summary.md"), summary)?;
		Ok(())
	}

	/// Generate summary content
	fn generate_summary(&self, snapshot_path: &Path) -> anyhow::Result<String> {
		let mut summary = String::new();

		summary.push_str(&format!("# Test Snapshot: {}\n\n", self.test_name));
		summary.push_str(&format!(
			"**Timestamp**: {}\n",
			Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
		));
		summary.push_str(&format!("**Test**: {}\n\n", self.test_name));

		summary.push_str("## Snapshot Contents\n\n");

		// List files in snapshot
		let files = self.list_snapshot_files(snapshot_path)?;
		for file in files {
			let metadata = fs::metadata(snapshot_path.join(&file))?;
			let size = if metadata.is_file() {
				format!(" ({} bytes)", metadata.len())
			} else {
				" (directory)".to_string()
			};
			summary.push_str(&format!("- {}{}\n", file, size));
		}

		summary.push_str("\n## Test Data Location\n\n");
		summary.push_str(&format!(
			"Temp directory: {}\n",
			self.test_data_path.display()
		));

		Ok(summary)
	}

	/// List all files in snapshot recursively
	fn list_snapshot_files(&self, path: &Path) -> anyhow::Result<Vec<String>> {
		let mut files = Vec::new();
		self.list_files_recursive(path, path, &mut files)?;
		files.sort();
		Ok(files)
	}

	fn list_files_recursive(
		&self,
		base: &Path,
		current: &Path,
		files: &mut Vec<String>,
	) -> anyhow::Result<()> {
		for entry in fs::read_dir(current)? {
			let entry = entry?;
			let path = entry.path();
			let relative = path.strip_prefix(base)?.to_string_lossy().to_string();

			files.push(relative.clone());

			if entry.file_type()?.is_dir() {
				self.list_files_recursive(base, &path, files)?;
			}
		}
		Ok(())
	}
}
