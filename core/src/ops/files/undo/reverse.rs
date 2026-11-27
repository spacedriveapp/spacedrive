//! What reversing a journal comes to, effect by effect.
//!
//! Effects reverse in the order opposite to the one they happened in. A
//! created path is trashed; a moved path goes back; a trashed item is put
//! back from where the trash reported it went; a replaced file gives way to
//! its previous bytes, kept in the trash; attributes go back to what they
//! were. A removal cannot be reversed, and neither can a trashing or a
//! replacement whose previous bytes were not kept. Before an effect is
//! reversed its subject is checked against what the journal recorded, so
//! undo never touches a file that changed since.
//!
//! An effect on a bucket object names it by its bucket URL and is checked
//! and reversed through the bucket's backend; a bucket whose volume is not
//! connected here leaves the effect as changed.

use std::path::{Path, PathBuf};

use crate::{
	infra::job::journal::{Effect, Recorded, Subject},
	ops::{
		files::bucket as files_bucket,
		grants::bucket::{backend, BucketUrl},
	},
	volume::{VolumeBackend, VolumeManager},
};

/// One step of an undo, as the plan and the job see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
	/// Trash what was created.
	Trash {
		path: PathBuf,
		is_dir: bool,
		size: u64,
	},
	/// Put a moved or trashed item back.
	MoveBack {
		from: PathBuf,
		to: PathBuf,
		size: u64,
	},
	/// Trash the replacement and put the previous bytes back.
	Restore {
		path: PathBuf,
		previous: PathBuf,
		size: u64,
	},
	/// Set attributes back.
	Attributes {
		path: PathBuf,
		to: crate::infra::job::journal::Attributes,
	},
}

/// Why an effect stays as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Left {
	/// Nothing recorded can reverse it.
	Irreversible,
	/// The subject is not what the journal recorded, or is gone.
	Changed,
	/// Something else now sits where the item would go back to.
	Occupied,
}

/// What reversing one effect comes to.
pub struct Reversal {
	pub sequence: i64,
	pub outcome: Result<Step, Left>,
}

/// Each effect's reversal, newest first, checked against the filesystem.
pub async fn reversals(
	journal: &[Recorded],
	only: Option<&[i64]>,
	volumes: Option<&VolumeManager>,
) -> Vec<Reversal> {
	let mut out = Vec::new();
	for recorded in journal.iter().rev() {
		if only.is_some_and(|only| !only.contains(&recorded.sequence)) {
			continue;
		}
		out.push(Reversal {
			sequence: recorded.sequence,
			outcome: match bucket_reverse(&recorded.effect, volumes).await {
				Some(outcome) => outcome,
				None => reverse(&recorded.effect).await,
			},
		});
	}
	out
}

async fn reverse(effect: &Effect) -> Result<Step, Left> {
	match effect {
		Effect::Created { path, subject } => {
			let subject = subject.ok_or(Left::Irreversible)?;
			if !subject.still_holds(path).await {
				return Err(Left::Changed);
			}
			Ok(Step::Trash {
				path: path.clone(),
				is_dir: subject.is_dir,
				size: subject.size,
			})
		}
		Effect::Moved { from, to, subject } => {
			if let Some(subject) = subject {
				if !subject.still_holds(to).await {
					return Err(Left::Changed);
				}
			} else if tokio::fs::symlink_metadata(to).await.is_err() {
				return Err(Left::Changed);
			}
			if occupied(from).await {
				return Err(Left::Occupied);
			}
			Ok(Step::MoveBack {
				from: to.clone(),
				to: from.clone(),
				size: subject.map(|subject| subject.size).unwrap_or(0),
			})
		}
		Effect::Trashed { from, to, subject } => {
			let to = to.as_ref().ok_or(Left::Irreversible)?;
			if let Some(subject) = subject {
				if !subject.still_holds(to).await {
					return Err(Left::Changed);
				}
			}
			if occupied(from).await {
				return Err(Left::Occupied);
			}
			Ok(Step::MoveBack {
				from: to.clone(),
				to: from.clone(),
				size: subject.map(|subject| subject.size).unwrap_or(0),
			})
		}
		Effect::Replaced {
			path,
			previous,
			subject,
		} => {
			let previous = previous.as_ref().ok_or(Left::Irreversible)?;
			if let Some(subject) = subject {
				if !subject.still_holds(path).await {
					return Err(Left::Changed);
				}
			}
			if tokio::fs::symlink_metadata(previous).await.is_err() {
				return Err(Left::Changed);
			}
			Ok(Step::Restore {
				path: path.clone(),
				previous: previous.clone(),
				size: subject.map(|subject| subject.size).unwrap_or(0),
			})
		}
		Effect::Removed { .. } => Err(Left::Irreversible),
		Effect::Attributes { path, from, .. } => {
			if tokio::fs::symlink_metadata(path).await.is_err() {
				return Err(Left::Changed);
			}
			Ok(Step::Attributes {
				path: path.clone(),
				to: from.clone(),
			})
		}
	}
}

/// The reversal of an effect on a bucket object, or `None` for an effect
/// on the filesystem.
async fn bucket_reverse(
	effect: &Effect,
	volumes: Option<&VolumeManager>,
) -> Option<Result<Step, Left>> {
	let url = BucketUrl::parse(effect.result()?)?;
	let Some(volumes) = volumes else {
		return Some(Err(Left::Changed));
	};
	let Ok(backend) = backend(volumes, &url).await else {
		return Some(Err(Left::Changed));
	};
	let backend = backend.as_ref();
	let holds = |key: String, subject: Option<Subject>| async move {
		match subject {
			Some(subject) => files_bucket::still_holds(backend, &key, &subject).await,
			None => matches!(files_bucket::object(backend, &key).await, Ok(Some(_))),
		}
	};
	Some(match effect {
		Effect::Created { path, subject } => {
			let Some(subject) = subject else {
				return Some(Err(Left::Irreversible));
			};
			if !holds(url.key.clone(), Some(*subject)).await {
				return Some(Err(Left::Changed));
			}
			Ok(Step::Trash {
				path: path.clone(),
				is_dir: false,
				size: subject.size,
			})
		}
		Effect::Moved { from, to, subject }
		| Effect::Trashed {
			from,
			to: Some(to),
			subject,
		} => {
			if !holds(url.key.clone(), *subject).await {
				return Some(Err(Left::Changed));
			}
			let Some(back) = BucketUrl::parse(from) else {
				return Some(Err(Left::Irreversible));
			};
			if files_bucket::occupied(backend, &back).await.unwrap_or(true) {
				return Some(Err(Left::Occupied));
			}
			Ok(Step::MoveBack {
				from: to.clone(),
				to: from.clone(),
				size: subject.map(|subject| subject.size).unwrap_or(0),
			})
		}
		_ => Err(Left::Irreversible),
	})
}

async fn occupied(path: &Path) -> bool {
	tokio::fs::symlink_metadata(path).await.is_ok()
}
