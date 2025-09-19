//! Strategy router for selecting the optimal copy method

use super::{
	input::CopyMethod,
	strategy::{
		CopyStrategy, FastCopyStrategy, LocalMoveStrategy, LocalStreamCopyStrategy,
		RemoteTransferStrategy,
	},
};
use crate::{domain::addressing::SdPath, volume::VolumeManager};
use std::sync::Arc;

pub struct CopyStrategyRouter;

impl CopyStrategyRouter {
	/// Selects the optimal copy strategy based on source, destination, and volume info
	pub async fn select_strategy(
		source: &SdPath,
		destination: &SdPath,
		is_move: bool,
		copy_method: &CopyMethod,
		volume_manager: Option<&VolumeManager>,
	) -> Box<dyn CopyStrategy> {
		// Cross-device transfer - always use network strategy
		if source.device_id() != destination.device_id() {
			return Box::new(RemoteTransferStrategy);
		}

		// For same-device operations, respect user's method preference
		match copy_method {
			CopyMethod::Atomic => {
				// User explicitly wants atomic operations
				if is_move {
					return Box::new(LocalMoveStrategy);
				} else {
					// For atomic copy, use fast copy strategy (std::fs::copy handles optimizations)
					return Box::new(FastCopyStrategy);
				}
			}
			CopyMethod::Streaming => {
				// User explicitly wants streaming copy
				return Box::new(LocalStreamCopyStrategy);
			}
			CopyMethod::Auto => {
				// Auto-select based on optimal strategy (original logic)
				// Same-device operation - get local paths for volume analysis
				let (source_path, dest_path) =
					match (source.as_local_path(), destination.as_local_path()) {
						(Some(s), Some(d)) => (s, d),
						_ => {
							// Fallback to streaming copy if paths aren't local
							return Box::new(LocalStreamCopyStrategy);
						}
					};

				// Check if paths are on the same volume
				let same_volume = if let Some(vm) = volume_manager {
					vm.same_volume(source_path, dest_path).await
				} else {
					// Fallback: if no volume manager, assume same-device local paths are same-volume
					Self::paths_likely_same_volume(source_path, dest_path)
				};

				if same_volume {
					// Same volume
					if is_move {
						// Use atomic move for same-volume moves
						return Box::new(LocalMoveStrategy);
					} else {
						// Same-volume copy - use fast copy strategy (std::fs::copy handles optimizations)
						return Box::new(FastCopyStrategy);
					}
				} else {
					// Cross-volume operation - use streaming copy
					return Box::new(LocalStreamCopyStrategy);
				}

				// Default to streaming copy for same-volume non-move operations
				Box::new(LocalStreamCopyStrategy)
			}
		}
	}

	/// Heuristic to determine if two local paths are likely on the same volume
	/// Used as fallback when VolumeManager is unavailable or incomplete
	fn paths_likely_same_volume(path1: &std::path::Path, path2: &std::path::Path) -> bool {
		// On macOS, paths under the same root are typically same volume
		#[cfg(target_os = "macos")]
		{
			// Both under /Users, /Applications, /System, etc. are likely same volume
			let common_roots = ["/Users", "/Applications", "/System", "/Library", "/private"];
			for root in &common_roots {
				if path1.starts_with(root) && path2.starts_with(root) {
					return true;
				}
			}
			// Both directly under / (like /tmp, /var) are likely same volume
			if path1.parent() == Some(std::path::Path::new("/"))
				&& path2.parent() == Some(std::path::Path::new("/"))
			{
				return true;
			}
		}

		// On Linux, similar heuristics
		#[cfg(target_os = "linux")]
		{
			let common_roots = ["/home", "/usr", "/var", "/opt", "/tmp"];
			for root in &common_roots {
				if path1.starts_with(root) && path2.starts_with(root) {
					return true;
				}
			}
		}

		// On Windows, same drive letter
		#[cfg(target_os = "windows")]
		{
			if let (Some(s1), Some(s2)) = (path1.to_str(), path2.to_str()) {
				if s1.len() >= 2 && s2.len() >= 2 {
					return s1.chars().nth(0) == s2.chars().nth(0)
						&& s1.chars().nth(1) == Some(':')
						&& s2.chars().nth(1) == Some(':');
				}
			}
		}

		false
	}

	/// Provides a human-readable description of the selected strategy
	pub async fn describe_strategy(
		source: &SdPath,
		destination: &SdPath,
		is_move: bool,
		copy_method: &CopyMethod,
		volume_manager: Option<&VolumeManager>,
	) -> String {
		if source.device_id() != destination.device_id() {
			return if is_move {
				"Cross-device move".to_string()
			} else {
				"Cross-device transfer".to_string()
			};
		}

		// For same-device operations, include user preference info
		let method_prefix = match copy_method {
			CopyMethod::Auto => "",
			CopyMethod::Atomic => "User-requested atomic ",
			CopyMethod::Streaming => "User-requested streaming ",
		};

		match copy_method {
			CopyMethod::Atomic => {
				if is_move {
					format!("{}move", method_prefix)
				} else {
					format!("{}fast copy", method_prefix)
				}
			}
			CopyMethod::Streaming => {
				if is_move {
					format!("{}move", method_prefix)
				} else {
					format!("{}copy", method_prefix)
				}
			}
			CopyMethod::Auto => {
				// Auto-select - use same logic as strategy selection
				let (source_path, dest_path) =
					match (source.as_local_path(), destination.as_local_path()) {
						(Some(s), Some(d)) => (s, d),
						_ => {
							return "Streaming copy".to_string();
						}
					};

				// Check if paths are on the same volume (same logic as select_strategy)
				let same_volume = if let Some(vm) = volume_manager {
					vm.same_volume(source_path, dest_path).await
				} else {
					Self::paths_likely_same_volume(source_path, dest_path)
				};

				if same_volume {
					if is_move {
						"Atomic move".to_string()
					} else {
						// Same-volume copy - use fast copy
						"Fast copy".to_string()
					}
				} else {
					if is_move {
						"Cross-volume move".to_string()
					} else {
						"Cross-volume streaming copy".to_string()
					}
				}
			}
		}
	}

	/// Estimates the performance characteristics of the selected strategy
	pub async fn estimate_performance(
		source: &SdPath,
		destination: &SdPath,
		is_move: bool,
		copy_method: &CopyMethod,
		volume_manager: Option<&VolumeManager>,
	) -> PerformanceEstimate {
		// Cross-device transfers always use network
		if source.device_id() != destination.device_id() {
			return PerformanceEstimate {
				speed_category: SpeedCategory::Network,
				supports_resume: true,
				requires_network: true,
				is_atomic: false,
			};
		}

		// For same-device operations, consider user's method preference
		match copy_method {
			CopyMethod::Atomic => {
				if is_move {
					PerformanceEstimate {
						speed_category: SpeedCategory::Instant,
						supports_resume: false,
						requires_network: false,
						is_atomic: true,
					}
				} else {
					// Fast copy operations (std::fs::copy with filesystem optimizations)
					PerformanceEstimate {
						speed_category: SpeedCategory::FastLocal,
						supports_resume: false,
						requires_network: false,
						is_atomic: true,
					}
				}
			}
			CopyMethod::Streaming => PerformanceEstimate {
				speed_category: SpeedCategory::LocalDisk,
				supports_resume: false,
				requires_network: false,
				is_atomic: false,
			},
			CopyMethod::Auto => {
				// Auto-select - use same logic as strategy selection
				let (source_path, dest_path) =
					match (source.as_local_path(), destination.as_local_path()) {
						(Some(s), Some(d)) => (s, d),
						_ => {
							return PerformanceEstimate {
								speed_category: SpeedCategory::LocalDisk,
								supports_resume: false,
								requires_network: false,
								is_atomic: false,
							};
						}
					};

				// Check if paths are on the same volume (same logic as select_strategy)
				let same_volume = if let Some(vm) = volume_manager {
					vm.same_volume(source_path, dest_path).await
				} else {
					Self::paths_likely_same_volume(source_path, dest_path)
				};

				if same_volume {
					if is_move {
						PerformanceEstimate {
							speed_category: SpeedCategory::Instant,
							supports_resume: false,
							requires_network: false,
							is_atomic: true,
						}
					} else {
						// Same-volume copy - use fast copy
						PerformanceEstimate {
							speed_category: SpeedCategory::FastLocal,
							supports_resume: false,
							requires_network: false,
							is_atomic: true,
						}
					}
				} else {
					// Cross-volume on same device
					PerformanceEstimate {
						speed_category: SpeedCategory::LocalDisk,
						supports_resume: true,
						requires_network: false,
						is_atomic: false,
					}
				}
			}
		}
	}
}

/// Performance characteristics of a copy strategy
#[derive(Debug, Clone)]
pub struct PerformanceEstimate {
	pub speed_category: SpeedCategory,
	pub supports_resume: bool,
	pub requires_network: bool,
	pub is_atomic: bool,
}

/// Categories of copy operation speed
#[derive(Debug, Clone, PartialEq)]
pub enum SpeedCategory {
	/// Instant operations (like atomic moves)
	Instant,
	/// Fast local operations (reflinks, same-volume copies)
	FastLocal,
	/// Regular disk-to-disk operations
	LocalDisk,
	/// Network transfers
	Network,
}
