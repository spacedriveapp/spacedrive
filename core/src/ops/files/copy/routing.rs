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
use tracing::info;

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
		info!("[ROUTING] Selecting strategy for source device {:?} -> destination device {:?}",
			source.device_id(), destination.device_id());

		// Cross-device transfer - always use network strategy
		if source.device_id() != destination.device_id() {
			info!("[ROUTING] Cross-device detected - selecting RemoteTransferStrategy");
			return Box::new(RemoteTransferStrategy);
		}

		info!("[ROUTING] Same device detected - selecting local strategy");

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

				// Check if paths are on the same physical storage (filesystem-aware)
				let same_storage = if let Some(vm) = volume_manager {
					vm.same_physical_storage(source_path, dest_path).await
				} else {
					// Fallback: if no volume manager, assume same-device local paths are different storage
					// This is safer than assuming same storage
					false
				};

				if same_storage {
					// Same physical storage - use filesystem-specific strategy
					if is_move {
						// Use atomic move for same-storage moves
						return Box::new(LocalMoveStrategy);
					} else {
						// Same-storage copy - check if filesystem supports CoW
						if let Some(vm) = volume_manager {
							if let Some(volume) = vm.volume_for_path(source_path).await {
								if volume.supports_fast_copy() {
									// Use fast copy for CoW filesystems (APFS, Btrfs, ZFS, ReFS)
									return Box::new(FastCopyStrategy);
								} else {
									// Non-CoW filesystem on same storage - use streaming
									return Box::new(LocalStreamCopyStrategy);
								}
							}
						}
						// Fallback to fast copy strategy when volume info unavailable
						return Box::new(FastCopyStrategy);
					}
				} else {
					// Cross-storage operation - use streaming copy
					return Box::new(LocalStreamCopyStrategy);
				}

				// Default to streaming copy for same-volume non-move operations
				Box::new(LocalStreamCopyStrategy)
			}
		}
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

				// Check if paths are on the same physical storage (same logic as select_strategy)
				let same_storage = if let Some(vm) = volume_manager {
					vm.same_physical_storage(source_path, dest_path).await
				} else {
					false // Conservative fallback
				};

				if same_storage {
					if is_move {
						"Atomic move (same storage)".to_string()
					} else {
						// Same-storage copy - describe based on filesystem
						if let Some(vm) = volume_manager {
							if let Some(volume) = vm.volume_for_path(source_path).await {
								return match volume.file_system {
									crate::volume::types::FileSystem::APFS => {
										"Fast copy (APFS clone)".to_string()
									}
									crate::volume::types::FileSystem::Btrfs => {
										"Fast copy (Btrfs reflink)".to_string()
									}
									crate::volume::types::FileSystem::ZFS => {
										"Fast copy (ZFS clone)".to_string()
									}
									crate::volume::types::FileSystem::ReFS => {
										"Fast copy (ReFS block clone)".to_string()
									}
									_ => "Fast copy (same storage)".to_string(),
								};
							}
						}
						"Fast copy (same storage)".to_string()
					}
				} else {
					if is_move {
						"Streaming move (cross-storage)".to_string()
					} else {
						"Streaming copy (cross-storage)".to_string()
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

				// Check if paths are on the same physical storage (same logic as select_strategy)
				let same_storage = if let Some(vm) = volume_manager {
					vm.same_physical_storage(source_path, dest_path).await
				} else {
					false // Conservative fallback
				};

				if same_storage {
					if is_move {
						PerformanceEstimate {
							speed_category: SpeedCategory::Instant,
							supports_resume: false,
							requires_network: false,
							is_atomic: true,
						}
					} else {
						// Same-storage copy - use fast copy (APFS clone, etc.)
						PerformanceEstimate {
							speed_category: SpeedCategory::Instant, // APFS clones are instant
							supports_resume: false,
							requires_network: false,
							is_atomic: true,
						}
					}
				} else {
					// Cross-storage on same device
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
