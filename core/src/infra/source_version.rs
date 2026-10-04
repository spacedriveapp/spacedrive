//! # Source content version
//!
//! `source_version` collapses a file's size and modification time into one
//! `u64` that changes whenever the file does. Caches keyed on a record's
//! identity carry it alongside, and compare it on read: equal means the cached
//! artifact still describes the bytes it was derived from, different means the
//! entry is stale and has to be regenerated.
//!
//! It lives here rather than in any one cache because more than one cache needs
//! the same answer. The thumbnail tile store and the mount block cache both
//! validate against it, and two definitions that drifted apart would show up as
//! one cache serving stale data while the other refreshed.
//!
//! This is a validator, not a content identity. It says "these bytes may have
//! changed", cheaply and without reading the file. It cannot say two files hold
//! the same content — that is what the convergent content uuid is for, and it
//! costs a hash.
//!
//! ## Example
//! ```rust
//! use sd_core::infra::source_version::source_version;
//! use std::time::{Duration, UNIX_EPOCH};
//!
//! let mtime = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
//! assert_eq!(source_version(1024, mtime), source_version(1024, mtime));
//! assert_ne!(source_version(1024, mtime), source_version(1025, mtime));
//! ```

use std::time::{SystemTime, UNIX_EPOCH};

/// Version word for a file, derived from its size and modification time.
///
/// Mixed rather than concatenated so that a small change in either input
/// changes the whole word: a truncated comparison still discriminates, and
/// callers that store the version in a fixed-width slot cannot accidentally
/// compare only the size half.
pub fn source_version(size: u64, mtime: SystemTime) -> u64 {
	let mtime_ns = mtime
		.duration_since(UNIX_EPOCH)
		.map_or(0, |d| d.as_nanos() as u64);
	splitmix64(size ^ splitmix64(mtime_ns))
}

/// SplitMix64 finalizer; full-avalanche mixing for [`source_version`].
fn splitmix64(mut x: u64) -> u64 {
	x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
	x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
	x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
	x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::time::Duration;

	#[test]
	fn tracks_size_and_mtime() {
		let mtime = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
		let v = source_version(1024, mtime);
		assert_eq!(v, source_version(1024, mtime));
		assert_ne!(v, source_version(1025, mtime));
		assert_ne!(v, source_version(1024, mtime + Duration::from_secs(1)));
	}

	#[test]
	fn a_time_before_the_epoch_does_not_panic() {
		let before = UNIX_EPOCH - Duration::from_secs(60);
		assert_eq!(source_version(1, before), source_version(1, UNIX_EPOCH));
	}
}
