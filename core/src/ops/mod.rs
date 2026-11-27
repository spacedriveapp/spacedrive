//! Operations module - contains all business operations and use cases
//!
//! This module organizes all business operations for Spacedrive:
//! - Addressing operations (path resolution)
//! - File operations (copy, move, delete, validate, duplicate detection)
//! - Indexing operations
//! - Media processing (thumbnails, etc.)
//! - Content operations (deduplication, statistics)
//! - Metadata operations (hierarchical tagging)

pub mod actors;
pub mod adapters;
pub mod addressing;
pub mod agent_write;
pub mod ai;
pub mod cli;
pub mod config;
// pub mod content;
pub mod core;
pub mod devices;
pub mod extension_test;
pub mod extensions;
pub mod files;
pub mod grants;
pub mod indexing;
pub mod jobs;
pub mod layouts;
pub mod libraries;
pub mod media;
pub mod models;
pub mod mounts;
pub mod navigation;
pub mod network;
pub mod paths;
pub mod processes;
#[cfg(feature = "video-proxies")]
pub mod proxies;
pub mod redundancy;
pub mod search;
pub mod sources;
pub mod spaces;
pub mod sync;
pub mod tags;
pub mod thumbs;
pub mod tools;
pub mod transcription;
pub mod volumes;
