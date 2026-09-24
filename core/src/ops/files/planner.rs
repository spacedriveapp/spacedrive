//! What copying one folder into another would do, read from the index.
//!
//! Source and destination stream against each other in key order through
//! the compare engine, directories and symlinks included, so each relative
//! path is met once with what sits there on both sides. A path only the
//! source has is created; a directory on both sides is merged into; two files
//! with the same bytes are a skip, a candidate one unless both sides have
//! been read in full; two files with different bytes are a collision the
//! policy resolves; and a file against a directory, or a link against a
//! file, is a conflict nothing resolves. Junk is never copied and never a
//! conflict. What only the destination holds is left out of the plan.
//!
//! Merge plans one pair per source folder. Copy and move plan the same pairs
//! one level down, each source written at its own name in the destination,
//! and note the root that is created or moved on top. Several pairs plan in
//! order, and a later one wanting a place an earlier one already claimed is
//! a conflict too, since the plan cannot say which should win.

use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
};

use sd_store::{FileKind, FsEntry};

use crate::{
	domain::SdPath,
	infra::{
		action::{error::ActionError, preflight::PreviewContext},
		query::QueryError,
	},
	ops::{
		files::{
			merge::MergeConflictPolicy,
			plan::{
				ChangeKind, ConflictKind, FsPlan, FsPlanSummary, PlanBasis, PlanChanges, PlanRoot,
				PlannedChange, ReplaceReason, SkipReason, StoreRevision,
			},
		},
		paths::compare::{CompareSet, Folder, Matcher, Sorted},
	},
};

/// Files no one wants carried along.
const JUNK: [&str; 3] = [".DS_Store", "Thumbs.db", "desktop.ini"];

pub(crate) fn is_junk(name: &str) -> bool {
	JUNK.contains(&name)
}

pub(crate) struct Planner<'a> {
	ctx: &'a PreviewContext,
	policy: MergeConflictPolicy,
	device: String,
	/// Destination-relative paths an earlier pair, or a renamed collision,
	/// already claims, keyed by destination root.
	claimed: HashSet<PathBuf>,
	summary: FsPlanSummary,
	changes: PlanChanges,
	revisions: Vec<StoreRevision>,
	roots: Vec<PlanRoot>,
	/// Every decision by destination-relative path, kept for a job that
	/// checks its own decisions against the plan's.
	decisions: Option<HashMap<String, ChangeKind>>,
	/// The files only the destination holds, by destination-relative path,
	/// after every pair so far: what a mirror removes. `None` until asked
	/// for.
	extras: Option<HashMap<String, FsEntry>>,
	/// Whether a pair has been planned, so the first pair's extras are the
	/// starting set rather than an intersection with nothing.
	paired: bool,
}

impl<'a> Planner<'a> {
	pub(crate) fn new(
		ctx: &'a PreviewContext,
		policy: MergeConflictPolicy,
		keep_decisions: bool,
	) -> Self {
		Self {
			ctx,
			policy,
			device: crate::device::get_current_device_slug(),
			claimed: HashSet::new(),
			summary: FsPlanSummary::default(),
			changes: PlanChanges::default(),
			revisions: Vec::new(),
			roots: Vec::new(),
			decisions: keep_decisions.then(HashMap::new),
			extras: None,
			paired: false,
		}
	}

	/// Plan the removal of what no source holds, after the pairs.
	pub(crate) fn removing_extras(mut self) -> Self {
		self.extras = Some(HashMap::new());
		self
	}

	pub(crate) fn summary(&self) -> &FsPlanSummary {
		&self.summary
	}

	/// A `Delete` row for each file the destination holds that no source
	/// does, flagged where it is the last copy of its bytes in the library.
	/// Called once, after every pair.
	pub(crate) async fn remove_extras(&mut self, dest_root: &Path) -> u64 {
		let Some(extras) = self.extras.take() else {
			return 0;
		};
		let hashes: Vec<String> = extras
			.values()
			.filter_map(|entry| entry.sampled_hash.clone())
			.collect();
		let last = crate::ops::files::delete::last_copies(self.ctx, hashes).await;
		let mut last_copies = 0;
		let mut paths: Vec<(String, FsEntry)> = extras.into_iter().collect();
		paths.sort_by(|a, b| a.0.cmp(&b.0));
		for (relative, entry) in paths {
			let last_copy = entry
				.sampled_hash
				.as_ref()
				.is_some_and(|hash| last.contains(hash));
			if last_copy {
				last_copies += 1;
			}
			let size = bytes(&entry);
			self.note(
				dest_root.join(&relative),
				ChangeKind::Delete { last_copy },
				size,
			);
		}
		last_copies
	}

	/// The plan so far, as a whole.
	pub(crate) fn finish(self) -> FsPlan {
		let (changes, truncated) = self.changes.finish();
		FsPlan {
			handle: None,
			basis: PlanBasis::Index {
				revisions: self.revisions,
			},
			roots: self.roots,
			summary: self.summary,
			changes,
			truncated,
		}
	}

	/// Every decision by destination-relative path.
	pub(crate) fn into_decisions(mut self) -> HashMap<String, ChangeKind> {
		self.decisions.take().unwrap_or_default()
	}

	/// A change at a place the walk does not visit: a root created, moved
	/// or removed on top of what happens beneath it.
	pub(crate) fn note(&mut self, path: PathBuf, change: ChangeKind, bytes: u64) {
		self.summary.count(&change, bytes);
		self.claimed.insert(path.clone());
		self.changes.push(PlannedChange {
			path: SdPath::Physical {
				device_slug: self.device.clone(),
				path,
			},
			change,
		});
	}

	/// Plan `source`'s files written beneath `destination`, both folders on
	/// this device, and remember the pair as one of the plan's roots.
	pub(crate) async fn pair(
		&mut self,
		source: &SdPath,
		destination: &SdPath,
		consumes: bool,
	) -> Result<(), ActionError> {
		let dest_root: PathBuf = destination
			.as_local_path()
			.ok_or_else(|| {
				ActionError::InvalidInput(format!("{destination} is not on this device"))
			})?
			.to_path_buf();
		self.roots.push(PlanRoot {
			source: source.clone(),
			destination: destination.clone(),
			consumes,
		});

		let from = self.ctx.reach(source).await;
		if from.is_empty() {
			return Err(ActionError::InvalidInput(format!(
				"{source} is not in a tracked source; track it to plan this"
			)));
		}
		let into = self.ctx.reach(destination).await;
		if into.is_empty() {
			return Err(ActionError::InvalidInput(format!(
				"{destination} is not in a tracked source; track it to plan this"
			)));
		}

		let mut a = Folder::open(self.ctx.index(), from, true, None)
			.await
			.map_err(read_failed)?
			.with_directories();
		let mut b = Folder::open(self.ctx.index(), into, true, None)
			.await
			.map_err(read_failed)?
			.with_directories();
		for (source, revision) in a
			.revisions()
			.await
			.map_err(read_failed)?
			.into_iter()
			.chain(b.revisions().await.map_err(read_failed)?)
		{
			if !self.revisions.iter().any(|known| known.source == source) {
				self.revisions.push(StoreRevision { source, revision });
			}
		}

		let mut matcher = Matcher::by_path(&mut a, &mut b);
		let mut only_here: HashMap<String, FsEntry> = HashMap::new();
		while let Some(sorted) = matcher.next().await.map_err(read_failed)? {
			let Some(relative) = sorted.path() else {
				continue;
			};
			if self.extras.is_some() && sorted.a.is_none() {
				if let Some(existing) = &sorted.b {
					if existing.entry.kind == FileKind::File && !is_junk(&existing.entry.name) {
						only_here.insert(relative.clone(), existing.entry.clone());
					}
				}
			}
			let Some((relative, change, bytes)) = self.decide(&dest_root, relative, sorted).await
			else {
				continue;
			};
			self.summary.count(&change, bytes);
			if let Some(decisions) = &mut self.decisions {
				decisions.insert(relative.clone(), change.clone());
			}
			self.changes.push(PlannedChange {
				path: SdPath::Physical {
					device_slug: self.device.clone(),
					path: dest_root.join(&relative),
				},
				change,
			});
		}
		// A file is an extra only if no source holds it: the first pair's
		// set, narrowed by each pair after.
		if let Some(extras) = &mut self.extras {
			if self.paired {
				extras.retain(|relative, _| only_here.contains_key(relative));
			} else {
				*extras = only_here;
			}
		}
		self.paired = true;
		Ok(())
	}

	/// What happens at one relative path, as the destination-relative path
	/// it is written to, the change, and the bytes the change is about. `None`
	/// where the plan has nothing to say.
	async fn decide(
		&mut self,
		dest_root: &Path,
		relative: String,
		sorted: Sorted,
	) -> Option<(String, ChangeKind, u64)> {
		let name = sorted.key()?.1.clone();
		let set = sorted.set;
		match (sorted.a, sorted.b) {
			// What only the destination holds stays as it is.
			(None, _) => None,
			(Some(incoming), None) => {
				if is_junk(&name) {
					return Some((relative, skip(SkipReason::Junk), 0));
				}
				if !self.claimed.insert(dest_root.join(&relative)) {
					return Some((relative, conflict(ConflictKind::Sources), 0));
				}
				Some(match incoming.entry.kind {
					FileKind::Directory => (relative, ChangeKind::CreateDirectory, 0),
					_ => {
						let size = bytes(&incoming.entry);
						(relative, ChangeKind::Create { size }, size)
					}
				})
			}
			(Some(incoming), Some(existing)) => {
				let (incoming, existing) = (incoming.entry, existing.entry);
				Some(match (incoming.kind, existing.kind) {
					(FileKind::Directory, FileKind::Directory) => {
						(relative, ChangeKind::MergeInto, 0)
					}
					(FileKind::Directory, _) | (_, FileKind::Directory) => {
						(relative, conflict(ConflictKind::FileVsDirectory), 0)
					}
					(FileKind::Symlink, FileKind::File) | (FileKind::File, FileKind::Symlink) => {
						(relative, conflict(ConflictKind::LinkVsFile), 0)
					}
					(FileKind::Symlink, FileKind::Symlink) => {
						if incoming.link_target == existing.link_target {
							(relative, skip(SkipReason::DuplicateConfirmed), 0)
						} else {
							self.collision(dest_root, relative, &incoming, &existing)
								.await
						}
					}
					(FileKind::File, FileKind::File) => {
						if is_junk(&name) {
							return Some((relative, skip(SkipReason::Junk), 0));
						}
						match set {
							CompareSet::Both => {
								let confirmed = matches!(
									(&incoming.integrity_hash, &existing.integrity_hash),
									(Some(ours), Some(theirs)) if ours == theirs
								);
								let reason = if confirmed {
									SkipReason::DuplicateConfirmed
								} else {
									SkipReason::DuplicateCandidate
								};
								let size = bytes(&incoming);
								(relative, skip(reason), size)
							}
							_ => {
								self.collision(dest_root, relative, &incoming, &existing)
									.await
							}
						}
					}
				})
			}
		}
	}

	/// The same path on both sides with different bytes, resolved as the
	/// policy says.
	async fn collision(
		&mut self,
		dest_root: &Path,
		relative: String,
		incoming: &FsEntry,
		existing: &FsEntry,
	) -> (String, ChangeKind, u64) {
		self.summary.collisions += 1;
		let (incoming_size, existing_size) = (bytes(incoming), bytes(existing));
		let replace = |reason| ChangeKind::Replace {
			existing_size,
			incoming_size,
			reason,
		};
		match self.policy {
			MergeConflictPolicy::Skip => (relative, skip(SkipReason::Policy), incoming_size),
			MergeConflictPolicy::Overwrite => {
				(relative, replace(ReplaceReason::Overwrite), incoming_size)
			}
			MergeConflictPolicy::KeepNewer => {
				if incoming.mtime_ms > existing.mtime_ms {
					(relative, replace(ReplaceReason::Newer), incoming_size)
				} else {
					(relative, skip(SkipReason::Policy), incoming_size)
				}
			}
			MergeConflictPolicy::KeepBoth => {
				let renamed = self.unique(dest_root, &relative).await;
				self.claimed.insert(dest_root.join(&renamed));
				(
					renamed,
					ChangeKind::Create {
						size: incoming_size,
					},
					incoming_size,
				)
			}
		}
	}

	/// A numbered name beside `path`, in the convention copy uses, that
	/// neither the index nor this plan holds.
	pub(crate) async fn unique_name(&self, path: &Path) -> PathBuf {
		let parent = path.parent().unwrap_or(Path::new(""));
		let name = path
			.file_name()
			.map(|name| name.to_string_lossy().into_owned())
			.unwrap_or_default();
		parent.join(self.unique(parent, &name).await)
	}

	/// A numbered name beside `relative`, in the convention copy uses, that
	/// neither the destination's index nor this plan holds.
	async fn unique(&self, dest_root: &Path, relative: &str) -> String {
		let (directory, name) = match relative.rsplit_once('/') {
			Some((directory, name)) => (Some(directory), name),
			None => (None, relative),
		};
		let (stem, extension) = match name.rsplit_once('.') {
			Some((stem, extension)) => (stem, format!(".{extension}")),
			None => (name, String::new()),
		};
		let store = self.ctx.index().store_for(dest_root).await;
		for counter in 1.. {
			let candidate = match directory {
				Some(directory) => format!("{directory}/{stem} ({counter}){extension}"),
				None => format!("{stem} ({counter}){extension}"),
			};
			if self.claimed.contains(&dest_root.join(&candidate)) {
				continue;
			}
			let held = match &store {
				Some(store) => store.contains_path(&dest_root.join(&candidate)).await,
				None => false,
			};
			if !held {
				return candidate;
			}
		}
		unreachable!("the counter never runs out")
	}
}

fn skip(reason: SkipReason) -> ChangeKind {
	ChangeKind::Skip { reason }
}

fn conflict(kind: ConflictKind) -> ChangeKind {
	ChangeKind::Conflict { kind }
}

fn bytes(entry: &FsEntry) -> u64 {
	entry.size.unwrap_or(0).max(0) as u64
}

fn read_failed(error: QueryError) -> ActionError {
	ActionError::Internal(error.to_string())
}
