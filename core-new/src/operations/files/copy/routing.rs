//! Strategy router for selecting the optimal copy method

use super::strategy::{CopyStrategy, LocalMoveStrategy, LocalStreamCopyStrategy, RemoteTransferStrategy};
use crate::{shared::types::SdPath, volume::VolumeManager};
use std::sync::Arc;

pub struct CopyStrategyRouter;

impl CopyStrategyRouter {
    /// Selects the optimal copy strategy based on source, destination, and volume info
    pub async fn select_strategy(
        source: &SdPath,
        destination: &SdPath,
        is_move: bool,
        volume_manager: Option<&VolumeManager>,
    ) -> Box<dyn CopyStrategy> {
        // Cross-device transfer
        if source.device_id != destination.device_id {
            return Box::new(RemoteTransferStrategy);
        }

        // Same-device operation - get local paths for volume analysis
        let (source_path, dest_path) = match (source.as_local_path(), destination.as_local_path()) {
            (Some(s), Some(d)) => (s, d),
            _ => {
                // Fallback to streaming copy if paths aren't local
                return Box::new(LocalStreamCopyStrategy);
            }
        };

        // Check if paths are on the same volume
        if let Some(vm) = volume_manager {
            if vm.same_volume(source_path, dest_path).await {
                // Same volume
                if is_move {
                    // Use atomic move for same-volume moves
                    return Box::new(LocalMoveStrategy);
                }
                // For same-volume copies, we could add optimized copy strategies here
                // (e.g., reflink on filesystems that support it)
                // For now, fall through to streaming copy
            }
        } else {
            // No volume manager available - make best guess
            // If it's a move operation on the same device, try atomic move
            if is_move {
                return Box::new(LocalMoveStrategy);
            }
        }

        // Default to streaming copy for cross-volume or non-move same-volume
        Box::new(LocalStreamCopyStrategy)
    }

    /// Provides a human-readable description of the selected strategy
    pub async fn describe_strategy(
        source: &SdPath,
        destination: &SdPath,
        is_move: bool,
        volume_manager: Option<&VolumeManager>,
    ) -> String {
        if source.device_id != destination.device_id {
            return if is_move {
                "Cross-device move".to_string()
            } else {
                "Cross-device transfer".to_string()
            };
        }

        // Same-device operation
        let (source_path, dest_path) = match (source.as_local_path(), destination.as_local_path()) {
            (Some(s), Some(d)) => (s, d),
            _ => {
                return "Streaming copy".to_string();
            }
        };

        if let Some(vm) = volume_manager {
            if vm.same_volume(source_path, dest_path).await {
                if is_move {
                    return "Atomic move".to_string();
                } else {
                    return "Same-volume copy".to_string();
                }
            } else {
                return if is_move {
                    "Cross-volume move".to_string()
                } else {
                    "Cross-volume streaming copy".to_string()
                };
            }
        }

        // Fallback description
        if is_move {
            "Local move".to_string()
        } else {
            "Local copy".to_string()
        }
    }

    /// Estimates the performance characteristics of the selected strategy
    pub async fn estimate_performance(
        source: &SdPath,
        destination: &SdPath,
        is_move: bool,
        volume_manager: Option<&VolumeManager>,
    ) -> PerformanceEstimate {
        if source.device_id != destination.device_id {
            return PerformanceEstimate {
                speed_category: SpeedCategory::Network,
                supports_resume: true,
                requires_network: true,
                is_atomic: false,
            };
        }

        let (source_path, dest_path) = match (source.as_local_path(), destination.as_local_path()) {
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

        if let Some(vm) = volume_manager {
            if vm.same_volume(source_path, dest_path).await {
                if is_move {
                    return PerformanceEstimate {
                        speed_category: SpeedCategory::Instant,
                        supports_resume: false,
                        requires_network: false,
                        is_atomic: true,
                    };
                } else {
                    // Could be optimized with filesystem features
                    let source_vol = vm.volume_for_path(source_path).await;
                    let supports_fast_copy = source_vol
                        .map(|v| v.supports_fast_copy())
                        .unwrap_or(false);

                    return PerformanceEstimate {
                        speed_category: if supports_fast_copy {
                            SpeedCategory::FastLocal
                        } else {
                            SpeedCategory::LocalDisk
                        },
                        supports_resume: false,
                        requires_network: false,
                        is_atomic: supports_fast_copy,
                    };
                }
            } else {
                // Cross-volume on same device
                let (source_vol, dest_vol) = (
                    vm.volume_for_path(source_path).await,
                    vm.volume_for_path(dest_path).await,
                );

                let estimated_speed = match (source_vol, dest_vol) {
                    (Some(s), Some(d)) => s.estimate_copy_speed(&d),
                    _ => None,
                };

                return PerformanceEstimate {
                    speed_category: SpeedCategory::LocalDisk,
                    supports_resume: true,
                    requires_network: false,
                    is_atomic: false,
                };
            }
        }

        // Fallback estimate
        PerformanceEstimate {
            speed_category: if is_move {
                SpeedCategory::FastLocal
            } else {
                SpeedCategory::LocalDisk
            },
            supports_resume: false,
            requires_network: false,
            is_atomic: is_move,
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