//! # Filesystem plans
//!
//! What a filesystem-mutating action would do, as a preview projects it from
//! the index: one change per path, complete counts, and the basis the plan
//! was read from. A plan is advisory. The filesystem is live, so a job applies
//! the same policy per leaf when it runs and reports where it diverged, and a
//! plan never overstates a hash: the index mostly holds sampled hashes, so a
//! duplicate can only be a candidate here, and only a job that has read every
//! byte calls it confirmed. Destructive decisions wait for that.
//!
//! The change list is bounded. Conflicts are always kept, up to the cap, and
//! other changes fill the remainder, since a person reading a plan needs the
//! conflicts more than the ten-thousandth create. The summary counts every
//! change whether or not it is listed.

use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
	time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{domain::SdPath, infra::job::journal::Attributes};

/// The most changes a plan lists. The summary counts past it.
pub const CHANGE_CAP: usize = 5000;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FsPlan {
	/// The handle the daemon keeps the plan under, for browsing it as an
	/// overlay on listings. Set by the preview that retained it.
	pub handle: Option<Uuid>,
	pub basis: PlanBasis,
	/// Each source folder the plan reads and where it is written, so a listing under a
	/// consumed source can show what leaves it.
	pub roots: Vec<PlanRoot>,
	pub summary: FsPlanSummary,
	pub changes: Vec<PlannedChange>,
	/// Whether changes were dropped from the list to keep it under the cap.
	pub truncated: bool,
}

/// One source folder of the plan and the folder it is written into.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PlanRoot {
	pub source: SdPath,
	pub destination: SdPath,
	/// Whether settled leaves are removed from the source.
	pub consumes: bool,
}

/// How long a retained plan outlives its last use.
const HANDLE_TTL: Duration = Duration::from_secs(10 * 60);

/// Plans the daemon keeps under handles, in memory only, for as long as
/// someone browses them. A lapsed handle is rebuilt by previewing again. A
/// handle only names a retained plan; execution takes the action's input,
/// never a handle.
#[derive(Default)]
pub struct PlanHandles {
	plans: Mutex<HashMap<Uuid, Retained>>,
}

struct Retained {
	plan: Arc<FsPlan>,
	touched: Instant,
}

impl PlanHandles {
	/// Keep a plan and hand it back naming its handle.
	pub fn retain(&self, mut plan: FsPlan) -> FsPlan {
		let handle = Uuid::new_v4();
		plan.handle = Some(handle);
		let mut plans = self.plans.lock().expect("plans");
		plans.retain(|_, retained| retained.touched.elapsed() < HANDLE_TTL);
		plans.insert(
			handle,
			Retained {
				plan: Arc::new(plan.clone()),
				touched: Instant::now(),
			},
		);
		plan
	}

	/// The plan under a handle, refreshed for another while.
	pub fn get(&self, handle: Uuid) -> Option<Arc<FsPlan>> {
		let mut plans = self.plans.lock().expect("plans");
		let retained = plans.get_mut(&handle)?;
		if retained.touched.elapsed() >= HANDLE_TTL {
			plans.remove(&handle);
			return None;
		}
		retained.touched = Instant::now();
		Some(retained.plan.clone())
	}
}

/// What a plan was read from.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlanBasis {
	/// The stores the plan read, at the revisions it read them.
	Index { revisions: Vec<StoreRevision> },
	/// A job's journal, for an undo.
	Journal { job: Uuid },
	/// An archive's own directory, for an extract.
	Archive { path: SdPath, entries: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct StoreRevision {
	pub source: Uuid,
	pub revision: i64,
}

/// One path the plan touches, at its place in the destination.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PlannedChange {
	pub path: SdPath,
	pub change: ChangeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChangeKind {
	Create {
		size: u64,
	},
	CreateDirectory,
	Replace {
		existing_size: u64,
		incoming_size: u64,
		reason: ReplaceReason,
	},
	/// A directory on both sides; the plan continues inside it.
	MergeInto,
	Skip {
		reason: SkipReason,
	},
	Move {
		from: SdPath,
	},
	Delete {
		last_copy: bool,
	},
	/// Nothing the policy resolves. Reported and left alone.
	Conflict {
		kind: ConflictKind,
	},
	/// The filesystem attributes that change, each absent where it stays.
	SetAttributes {
		attributes: Attributes,
	},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ReplaceReason {
	/// The policy replaces whatever differs.
	Overwrite,
	/// The incoming file's modification time is later, which is a claim the
	/// filesystem makes rather than proof about the bytes.
	Newer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
	/// The same bytes by sampled hash, or by size and modification time
	/// where a side is unhashed. A job reads both in full before it acts on
	/// this.
	DuplicateCandidate,
	/// The same bytes by integrity hash on both sides.
	DuplicateConfirmed,
	/// A file no one wants copied: `.DS_Store`, `Thumbs.db`, `desktop.ini`.
	Junk,
	/// The policy leaves the existing file as it is.
	Policy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
	/// A file on one side where the other has a directory.
	FileVsDirectory,
	/// A symlink on one side where the other has a regular file.
	LinkVsFile,
	/// Two sources of one operation want the same place.
	Sources,
}

/// Complete counts and bytes per change kind, whether or not a change made
/// the list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FsPlanSummary {
	pub creates: Tally,
	pub directories_created: u64,
	pub replaces: Tally,
	pub merged_into: u64,
	pub skips: Skips,
	pub moves: Tally,
	pub deletes: Tally,
	pub conflicts: u64,
	/// Files at the same path on both sides with different bytes, which the
	/// policy resolved one way or another.
	pub collisions: u64,
	pub attributes: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Tally {
	pub files: u64,
	pub bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Skips {
	pub duplicate_candidates: Tally,
	pub duplicates_confirmed: Tally,
	pub junk: u64,
	pub policy: Tally,
}

impl Tally {
	fn add(&mut self, bytes: u64) {
		self.files += 1;
		self.bytes += bytes;
	}
}

impl FsPlanSummary {
	/// Count a change, with the bytes it moves or leaves in place.
	pub fn count(&mut self, change: &ChangeKind, bytes: u64) {
		match change {
			ChangeKind::Create { .. } => self.creates.add(bytes),
			ChangeKind::CreateDirectory => self.directories_created += 1,
			ChangeKind::Replace { .. } => self.replaces.add(bytes),
			ChangeKind::MergeInto => self.merged_into += 1,
			ChangeKind::Skip { reason } => match reason {
				SkipReason::DuplicateCandidate => self.skips.duplicate_candidates.add(bytes),
				SkipReason::DuplicateConfirmed => self.skips.duplicates_confirmed.add(bytes),
				SkipReason::Junk => self.skips.junk += 1,
				SkipReason::Policy => self.skips.policy.add(bytes),
			},
			ChangeKind::Move { .. } => self.moves.add(bytes),
			ChangeKind::Delete { .. } => self.deletes.add(bytes),
			ChangeKind::Conflict { .. } => self.conflicts += 1,
			ChangeKind::SetAttributes { .. } => self.attributes += 1,
		}
	}

	/// The bytes the destination gains: creates and replaced growth.
	pub fn bytes_needed(&self) -> u64 {
		self.creates.bytes + self.replaces.bytes
	}
}

/// The change list as a plan gathers it: conflicts kept apart so they are
/// never the ones dropped.
#[derive(Default)]
pub struct PlanChanges {
	conflicts: Vec<PlannedChange>,
	others: Vec<PlannedChange>,
	truncated: bool,
}

impl PlanChanges {
	pub fn push(&mut self, change: PlannedChange) {
		let full = self.conflicts.len() + self.others.len() >= CHANGE_CAP;
		if matches!(change.change, ChangeKind::Conflict { .. }) {
			// A conflict displaces another change rather than being dropped,
			// until the cap holds nothing but conflicts.
			if full {
				self.truncated = true;
				if self.others.pop().is_none() {
					return;
				}
			}
			self.conflicts.push(change);
		} else if full {
			self.truncated = true;
		} else {
			self.others.push(change);
		}
	}

	/// The list, conflicts first, and whether anything was dropped.
	pub fn finish(mut self) -> (Vec<PlannedChange>, bool) {
		self.conflicts.append(&mut self.others);
		(self.conflicts, self.truncated)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn change(change: ChangeKind) -> PlannedChange {
		PlannedChange {
			path: SdPath::local("/x"),
			change,
		}
	}

	/// Past the cap, conflicts are kept and other changes are dropped, and
	/// the plan says so.
	#[test]
	fn conflicts_survive_the_cap() {
		let mut changes = PlanChanges::default();
		for _ in 0..CHANGE_CAP {
			changes.push(change(ChangeKind::Create { size: 1 }));
		}
		changes.push(change(ChangeKind::Conflict {
			kind: ConflictKind::Sources,
		}));
		let (listed, truncated) = changes.finish();
		assert_eq!(listed.len(), CHANGE_CAP);
		assert!(truncated);

		let mut changes = PlanChanges::default();
		changes.push(change(ChangeKind::Create { size: 1 }));
		changes.push(change(ChangeKind::Conflict {
			kind: ConflictKind::Sources,
		}));
		let (listed, truncated) = changes.finish();
		assert!(matches!(listed[0].change, ChangeKind::Conflict { .. }));
		assert!(!truncated);
	}

	/// A retained plan comes back under its handle until it lapses.
	#[test]
	fn a_handle_names_a_retained_plan() {
		let handles = PlanHandles::default();
		let plan = handles.retain(FsPlan {
			handle: None,
			basis: PlanBasis::Index {
				revisions: Vec::new(),
			},
			roots: Vec::new(),
			summary: FsPlanSummary::default(),
			changes: vec![change(ChangeKind::CreateDirectory)],
			truncated: false,
		});
		let handle = plan.handle.expect("a handle");
		assert_eq!(handles.get(handle).expect("kept").changes.len(), 1);
		assert!(handles.get(Uuid::new_v4()).is_none());
	}

	#[test]
	fn a_summary_counts_every_kind() {
		let mut summary = FsPlanSummary::default();
		summary.count(&ChangeKind::Create { size: 5 }, 5);
		summary.count(&ChangeKind::CreateDirectory, 0);
		summary.count(
			&ChangeKind::Replace {
				existing_size: 1,
				incoming_size: 7,
				reason: ReplaceReason::Overwrite,
			},
			7,
		);
		summary.count(&ChangeKind::MergeInto, 0);
		summary.count(
			&ChangeKind::Skip {
				reason: SkipReason::DuplicateCandidate,
			},
			3,
		);
		summary.count(
			&ChangeKind::Skip {
				reason: SkipReason::Junk,
			},
			0,
		);
		summary.count(
			&ChangeKind::Conflict {
				kind: ConflictKind::FileVsDirectory,
			},
			0,
		);
		assert_eq!(summary.creates, Tally { files: 1, bytes: 5 });
		assert_eq!(summary.directories_created, 1);
		assert_eq!(summary.replaces, Tally { files: 1, bytes: 7 });
		assert_eq!(summary.merged_into, 1);
		assert_eq!(
			summary.skips.duplicate_candidates,
			Tally { files: 1, bytes: 3 }
		);
		assert_eq!(summary.skips.junk, 1);
		assert_eq!(summary.conflicts, 1);
		assert_eq!(summary.bytes_needed(), 12);
	}
}
