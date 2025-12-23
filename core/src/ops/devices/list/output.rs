//! Output types for library devices query
//!
//! Note: The canonical type for devices is `crate::domain::Device`.
//! This module re-exports it for backwards compatibility.

/// Device information type alias
///
/// The canonical device type is `crate::domain::Device`, which is used
/// for both database-registered devices and network-paired devices.
pub type LibraryDeviceInfo = crate::domain::Device;
