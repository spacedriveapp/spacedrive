//! Operations module - contains all business operations and use cases
//!
//! This module organizes all business operations for Spacedrive:
//! - Addressing operations (path resolution)
//! - File operations (copy, move, delete, validate, duplicate detection)
//! - Indexing operations
//! - Media processing (thumbnails, etc.)
//! - Content operations (deduplication, statistics)
//! - Metadata operations (hierarchical tagging)

pub mod addressing;
// pub mod content;
pub mod core;
pub mod devices;
pub mod extension_test;
pub mod files;
pub mod indexing;
pub mod jobs;
pub mod libraries;
pub mod locations;
pub mod media;
pub mod metadata;
pub mod models;
pub mod network;
pub mod search;
pub mod sidecar;
pub mod spaces;
pub mod sync;
pub mod tags;
pub mod volumes;
