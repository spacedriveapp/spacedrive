//! # Live content search
//!
//! `search.grep` runs a regex or literal search over the files under one path,
//! on the device that holds the bytes. It reads files as they are on disk and
//! needs no index, so it answers for a source the moment it is tracked and
//! never returns text that has since changed.
//!
//! The scan itself, [`scan::grep`], takes a plain filesystem path. The query
//! resolves an `SdPath` to a path under a source tracked on this device and
//! hands it over; any layer that has already decided which subtree a caller
//! may read can pass that subtree straight in.
//!
//! Results come back a page at a time. The walk visits files in a fixed order,
//! so a cursor naming the last file and how many of its matches were returned
//! resumes exactly where a page stopped, and no response grows with the size
//! of the tree.
//!
//! ## Example
//! ```rust,no_run
//! use std::{path::Path, sync::atomic::AtomicBool};
//! use sd_core::ops::search::grep::{scan, GrepOptions};
//!
//! let options = GrepOptions::new("fn main");
//! let page = scan::grep(Path::new("/home/me/code"), &options, &AtomicBool::new(false))?;
//! for m in &page.matches {
//! 	tracing::info!("{}:{}: {}", m.path, m.line_number, m.line);
//! }
//! # Ok::<(), sd_core::ops::search::grep::scan::GrepError>(())
//! ```

pub mod input;
pub mod output;
pub mod query;
pub mod scan;

pub use input::*;
pub use output::*;
pub use query::*;
