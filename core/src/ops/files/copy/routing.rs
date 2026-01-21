//! # Strategy Router
//!
//! Selects the optimal copy strategy based on source, destination, and volume info.
//! Exposes metadata about the selected strategy for UI display.

use super::{
	input::CopyMethod,
	strategy::{
		CopyStrategy, FastCopyStrategy, LocalMoveStrategy, LocalStreamCopyStrategy,
		RemoteTransferStrategy,
	},
};
use crate::{domain::addressing::SdPath, volume::VolumeManager};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use tracing::info;

/// Metadata about the selected copy strategy for UI display.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CopyStrategyMetadata {
	/// Internal strategy name (e.g., "LocalMove", "FastCopy", "LocalStream", "RemoteTransfer")
	pub strategy_name: String,
	/// Human-readable description (e.g., "Atomic move (same storage)")
	pub strategy_description: String,
	/// Whether operation crosses device boundaries
	pub is_cross_device: bool,
	/// Whether operation crosses volume/partition boundaries on same device
	pub is_cross_volume: bool,
	/// Whether this is expected to be a fast operation (instant or near-instant)
	pub is_fast_operation: bool,
	/// The copy method used (Auto, Atomic, Streaming)
	pub copy_method: CopyMethod,
}

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
		info!(
			"[ROUTING] Selecting strategy for source device {:?} -> destination device {:?}",
			source.device_slug(),
			destination.device_slug()
		);

		// Cross-device transfer - always use network strategy
		// Compare device slugs to detect if paths are on different devices
		let is_cross_device = match (source.device_slug(), destination.device_slug()) {
			(Some(src_slug), Some(dst_slug)) => src_slug != dst_slug,
			_ => false, // If either is None (cloud/content paths), not cross-device
		};

		if is_cross_device {
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

	/// Select strategy with full metadata about the operation.
	/// Returns both the strategy and metadata for UI display.
	pub async fn select_strategy_with_metadata(
		source: &SdPath,
		destination: &SdPath,
		is_move: bool,
		copy_method: &CopyMethod,
		volume_manager: Option<&VolumeManager>,
	) -> (Box<dyn CopyStrategy>, CopyStrategyMetadata) {
		let is_cross_device = match (source.device_slug(), destination.device_slug()) {
			(Some(src_slug), Some(dst_slug)) => src_slug != dst_slug,
			_ => false,
		};

		if is_cross_device {
			let description = if is_move {
				"Cross-device move (network transfer)".to_string()
			} else {
				"Cross-device copy (network transfer)".to_string()
			};
			let metadata = CopyStrategyMetadata {
				strategy_name: "RemoteTransfer".to_string(),
				strategy_description: description,
				is_cross_device: true,
				is_cross_volume: true,
				is_fast_operation: false,
				copy_method: copy_method.clone(),
			};
			return (Box::new(RemoteTransferStrategy), metadata);
		}

		// Same device - check storage topology
		let (source_path, dest_path) = match (source.as_local_path(), destination.as_local_path()) {
			(Some(s), Some(d)) => (s, d),
			_ => {
				let metadata = CopyStrategyMetadata {
					strategy_name: "LocalStream".to_string(),
					strategy_description: "Streaming copy".to_string(),
					is_cross_device: false,
					is_cross_volume: false,
					is_fast_operation: false,
					copy_method: copy_method.clone(),
				};
				return (Box::new(LocalStreamCopyStrategy), metadata);
			}
		};

		let same_storage = if let Some(vm) = volume_manager {
			vm.same_physical_storage(source_path, dest_path).await
		} else {
			false
		};

		let same_volume = if let Some(vm) = volume_manager {
			vm.same_volume(source_path, dest_path).await
		} else {
			false
		};

		match copy_method {
			CopyMethod::Atomic => {
				if is_move {
					let metadata = CopyStrategyMetadata {
						strategy_name: "LocalMove".to_string(),
						strategy_description: "User-requested atomic move".to_string(),
						is_cross_device: false,
						is_cross_volume: !same_volume,
						is_fast_operation: same_volume,
						copy_method: CopyMethod::Atomic,
					};
					(Box::new(LocalMoveStrategy), metadata)
				} else {
					let metadata = CopyStrategyMetadata {
						strategy_name: "FastCopy".to_string(),
						strategy_description: "User-requested fast copy".to_string(),
						is_cross_device: false,
						is_cross_volume: !same_volume,
						is_fast_operation: same_storage,
						copy_method: CopyMethod::Atomic,
					};
					(Box::new(FastCopyStrategy), metadata)
				}
			}
			CopyMethod::Streaming => {
				let metadata = CopyStrategyMetadata {
					strategy_name: "LocalStream".to_string(),
					strategy_description: if is_move {
						"User-requested streaming move".to_string()
					} else {
						"User-requested streaming copy".to_string()
					},
					is_cross_device: false,
					is_cross_volume: !same_volume,
					is_fast_operation: false,
					copy_method: CopyMethod::Streaming,
				};
				(Box::new(LocalStreamCopyStrategy), metadata)
			}
			CopyMethod::Auto => {
				if same_storage {
					if is_move {
						let metadata = CopyStrategyMetadata {
							strategy_name: "LocalMove".to_string(),
							strategy_description: "Atomic move (same storage)".to_string(),
							is_cross_device: false,
							is_cross_volume: false,
							is_fast_operation: true,
							copy_method: CopyMethod::Auto,
						};
						(Box::new(LocalMoveStrategy), metadata)
					} else {
						// Check for CoW filesystem
						let (strategy, description, is_fast): (
							Box<dyn CopyStrategy>,
							String,
							bool,
						) = if let Some(vm) = volume_manager {
							if let Some(volume) = vm.volume_for_path(source_path).await {
								if volume.supports_fast_copy() {
									let desc = match volume.file_system {
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
									(Box::new(FastCopyStrategy), desc, true)
								} else {
									(
										Box::new(LocalStreamCopyStrategy),
										"Streaming copy (same storage, no CoW)".to_string(),
										false,
									)
								}
							} else {
								(
									Box::new(FastCopyStrategy),
									"Fast copy (same storage)".to_string(),
									true,
								)
							}
						} else {
							(
								Box::new(FastCopyStrategy),
								"Fast copy (same storage)".to_string(),
								true,
							)
						};

						let metadata = CopyStrategyMetadata {
							strategy_name: if is_fast {
								"FastCopy".to_string()
							} else {
								"LocalStream".to_string()
							},
							strategy_description: description,
							is_cross_device: false,
							is_cross_volume: false,
							is_fast_operation: is_fast,
							copy_method: CopyMethod::Auto,
						};
						(strategy, metadata)
					}
				} else {
					// Cross-storage on same device
					let description = if is_move {
						"Streaming move (cross-storage)".to_string()
					} else {
						"Streaming copy (cross-storage)".to_string()
					};
					let metadata = CopyStrategyMetadata {
						strategy_name: "LocalStream".to_string(),
						strategy_description: description,
						is_cross_device: false,
						is_cross_volume: true,
						is_fast_operation: false,
						copy_method: CopyMethod::Auto,
					};
					(Box::new(LocalStreamCopyStrategy), metadata)
				}
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
		// Check if cross-device using device slugs
		let is_cross_device = match (source.device_slug(), destination.device_slug()) {
			(Some(src_slug), Some(dst_slug)) => src_slug != dst_slug,
			_ => false,
		};

		if is_cross_device {
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
		let is_cross_device = match (source.device_slug(), destination.device_slug()) {
			(Some(src_slug), Some(dst_slug)) => src_slug != dst_slug,
			_ => false,
		};

		if is_cross_device {
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
