//! Translating between the spelling a watch is stored under and the spellings
//! the platform will actually deliver events for.
//!
//! macOS reaches the writable half of the disk twice. `/Users/me` and
//! `/System/Volumes/Data/Users/me` are the same directory: same device, same
//! inode, joined by a firmlink. `realpath` does not resolve firmlinks, so both
//! spellings survive canonicalization, and FSEvents subscribes to exactly one
//! of them. Registered the long way round it accepts the watch, reports no
//! error, and delivers nothing.
//!
//! The mapping is not guesswork: `/usr/share/firmlinks` publishes it, one line
//! per link, root-volume path and Data-volume-relative path separated by a tab.
//!
//! Everything above this module works in one spelling — the one the source
//! stores, which is the long one, because a whole-drive source is rooted at
//! `/System/Volumes/Data` and its records have to sit under their own root.
//! [`subscriptions`] translates outbound, at registration, and [`restore`]
//! translates back inbound, so a path that leaves in FSEvents' spelling returns
//! in Spacedrive's.

use std::path::{Path, PathBuf};

/// One platform subscription backing a watch.
///
/// A watch is usually one of these. It is more than one when the requested root
/// spans several firmlinks, which is the whole-drive case: `/System/Volumes/Data`
/// has no single short spelling, so it fans out into the links beneath it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
	/// The path handed to the platform watcher.
	pub subscribe: PathBuf,
	/// The prefix an event arriving under `subscribe` is rewritten back to.
	pub restore_to: PathBuf,
}

impl Subscription {
	/// A subscription that needs no translation.
	fn direct(path: &Path) -> Self {
		Self {
			subscribe: path.to_path_buf(),
			restore_to: path.to_path_buf(),
		}
	}
}

/// Rewrite an event path from the spelling it arrived in to the spelling the
/// watch is stored under.
///
/// Returns `None` when the path is not under this subscription, which is the
/// ordinary answer while looking for the one that owns it.
pub fn restore(subscription: &Subscription, path: &Path) -> Option<PathBuf> {
	let rest = path.strip_prefix(&subscription.subscribe).ok()?;
	Some(subscription.restore_to.join(rest))
}

/// The platform subscriptions a watch on `path` requires.
///
/// Never empty: a path with no firmlink translation subscribes to itself.
pub fn subscriptions(path: &Path) -> Vec<Subscription> {
	#[cfg(target_os = "macos")]
	{
		macos::subscriptions(path)
	}
	#[cfg(not(target_os = "macos"))]
	{
		vec![Subscription::direct(path)]
	}
}

#[cfg(target_os = "macos")]
mod macos {
	use super::Subscription;
	use std::path::{Path, PathBuf};
	use std::sync::OnceLock;

	/// Where the Data volume mounts, and the root of every long spelling.
	const DATA_VOLUME: &str = "/System/Volumes/Data";
	const FIRMLINKS: &str = "/usr/share/firmlinks";

	/// One published firmlink: a path on the root volume, and the same
	/// directory's path relative to the Data volume root.
	struct Firmlink {
		root: PathBuf,
		relative: PathBuf,
	}

	fn table() -> &'static [Firmlink] {
		static TABLE: OnceLock<Vec<Firmlink>> = OnceLock::new();
		TABLE.get_or_init(|| {
			let Ok(contents) = std::fs::read_to_string(FIRMLINKS) else {
				return Vec::new();
			};
			contents
				.lines()
				.filter_map(|line| {
					let (root, relative) = line.split_once('\t')?;
					if root.is_empty() || relative.is_empty() {
						return None;
					}
					Some(Firmlink {
						root: PathBuf::from(root),
						relative: PathBuf::from(relative),
					})
				})
				.collect()
		})
	}

	/// Whether two paths are the same directory, so a rewrite can never land
	/// somewhere else because the published table disagreed with the disk.
	fn same_directory(a: &Path, b: &Path) -> bool {
		use std::os::unix::fs::MetadataExt;
		match (std::fs::metadata(a), std::fs::metadata(b)) {
			(Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
			_ => false,
		}
	}

	pub(super) fn subscriptions(path: &Path) -> Vec<Subscription> {
		let Ok(rest) = path.strip_prefix(DATA_VOLUME) else {
			return vec![Subscription::direct(path)];
		};

		let mut found = Vec::new();

		for link in table() {
			// The requested root sits inside this firmlink: one subscription,
			// the short spelling of the same directory.
			if let Ok(below) = rest.strip_prefix(&link.relative) {
				let subscribe = link.root.join(below);
				if same_directory(&subscribe, path) {
					return vec![Subscription {
						subscribe,
						restore_to: path.to_path_buf(),
					}];
				}
				continue;
			}

			// The firmlink sits inside the requested root: part of a fan-out.
			if link.relative.starts_with(rest) {
				let restore_to = Path::new(DATA_VOLUME).join(&link.relative);
				if same_directory(&link.root, &restore_to) {
					found.push(Subscription {
						subscribe: link.root.clone(),
						restore_to,
					});
				}
			}
		}

		if found.is_empty() {
			// A Data-volume path outside every firmlink is only reachable the
			// long way round, so watch it as asked.
			return vec![Subscription::direct(path)];
		}

		found
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_path_with_no_translation_watches_itself() {
		let path = Path::new("/Volumes/Archive/Photos");
		assert_eq!(subscriptions(path), vec![Subscription::direct(path)]);
	}

	#[test]
	fn an_event_is_rewritten_into_the_stored_spelling() {
		let subscription = Subscription {
			subscribe: PathBuf::from("/Users/me/Desktop"),
			restore_to: PathBuf::from("/System/Volumes/Data/Users/me/Desktop"),
		};

		assert_eq!(
			restore(&subscription, Path::new("/Users/me/Desktop/shot.png")),
			Some(PathBuf::from(
				"/System/Volumes/Data/Users/me/Desktop/shot.png"
			))
		);
		assert_eq!(
			restore(&subscription, Path::new("/Users/me/Desktop")),
			Some(PathBuf::from("/System/Volumes/Data/Users/me/Desktop"))
		);
	}

	#[test]
	fn an_event_outside_a_subscription_is_not_claimed() {
		let subscription = Subscription {
			subscribe: PathBuf::from("/Users/me/Desktop"),
			restore_to: PathBuf::from("/System/Volumes/Data/Users/me/Desktop"),
		};

		assert_eq!(
			restore(&subscription, Path::new("/Users/me/Documents/notes.txt")),
			None
		);
	}

	/// A path that merely begins with the same bytes is a different directory
	/// and must not be rewritten into one.
	#[test]
	fn a_lookalike_prefix_is_left_alone() {
		let impostor = Path::new("/System/Volumes/Data-not-really/Users");
		assert_eq!(
			subscriptions(impostor),
			vec![Subscription::direct(impostor)]
		);
	}

	#[cfg(target_os = "macos")]
	#[test]
	fn a_long_spelling_subscribes_to_the_short_one() {
		let long = Path::new("/System/Volumes/Data/Users");
		let subs = subscriptions(long);

		assert_eq!(subs.len(), 1, "one directory, one subscription");
		assert_eq!(subs[0].subscribe, Path::new("/Users"));
		assert_eq!(subs[0].restore_to, long);
	}

	/// The whole Data volume has no short spelling of its own, so a watch on it
	/// only reaches the disk by subscribing to each firmlink beneath it.
	#[cfg(target_os = "macos")]
	#[test]
	fn the_whole_data_volume_fans_out_into_its_firmlinks() {
		let subs = subscriptions(Path::new("/System/Volumes/Data"));

		assert!(
			subs.len() > 1,
			"expected a fan-out, got {:?}",
			subs.iter().map(|s| &s.subscribe).collect::<Vec<_>>()
		);
		assert!(
			subs.iter().any(|s| s.subscribe == Path::new("/Users")),
			"the home directories are the point of this watch"
		);
		for subscription in &subs {
			assert!(
				subscription.restore_to.starts_with("/System/Volumes/Data"),
				"{:?} would deliver events outside the watch",
				subscription
			);
		}
	}

	/// The round trip is what the watcher relies on: subscribe under one
	/// spelling, receive under it, hand back the other.
	#[cfg(target_os = "macos")]
	#[test]
	fn a_fanned_out_event_returns_in_the_stored_spelling() {
		let subs = subscriptions(Path::new("/System/Volumes/Data"));
		let users = subs
			.iter()
			.find(|s| s.subscribe == Path::new("/Users"))
			.expect("the /Users firmlink");

		assert_eq!(
			restore(users, Path::new("/Users/me/Desktop/shot.png")),
			Some(PathBuf::from(
				"/System/Volumes/Data/Users/me/Desktop/shot.png"
			))
		);
	}
}
