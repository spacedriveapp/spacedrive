//! Each target's new name, checked against the directory it sits in.
//!
//! Preflight and the job resolve the same way, so the job refuses exactly
//! what validation would have. Every check reads the live directory once
//! per parent: the sibling names, and whether the directory matches names
//! without regard to case, which is probed by asking for a sibling's name
//! with its case flipped rather than trusted from the filesystem's name,
//! since a volume can be formatted either way.

use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
};

use chrono::{DateTime, Local};

use super::{
	naming::{check_name, check_portable, NameRules},
	rules::{apply, RenameRule, Target},
};
use crate::{
	domain::SdPath,
	infra::action::preflight::Finding,
	ops::{indexing::VolumeIndex, paths::reach::stores_beneath_in},
	volume::VolumeManager,
};

pub const REMOTE_ROOT: &str = "rename.remote_root";
pub const MISSING: &str = "rename.missing";
pub const ILLEGAL_NAME: &str = "rename.illegal_name";
pub const EXISTS: &str = "rename.exists";
pub const CASE_COLLISION: &str = "rename.case_collision";
pub const CASE_ONLY: &str = "rename.case_only";
pub const COLLISION: &str = "rename.collision";
pub const UNCHANGED: &str = "rename.unchanged";
pub const RULE: &str = "rename.rule";

/// What is being renamed: names given one by one, or targets and the
/// rules that name them.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum RenameSet {
	Named(Vec<(SdPath, String)>),
	Ruled {
		targets: Vec<SdPath>,
		rules: Vec<RenameRule>,
	},
}

impl RenameSet {
	pub fn targets(&self) -> Vec<&SdPath> {
		match self {
			Self::Named(pairs) => pairs.iter().map(|(target, _)| target).collect(),
			Self::Ruled { targets, .. } => targets.iter().collect(),
		}
	}
}

/// One rename the set resolves to.
#[derive(Debug, Clone)]
pub struct Rename {
	pub from: PathBuf,
	pub to: PathBuf,
	pub is_dir: bool,
	pub size: u64,
	/// The new name differs from the old in case alone, on a directory that
	/// matches names without regard to it.
	pub case_only: bool,
	/// Whether a finding refuses this rename.
	pub refused: bool,
	/// Another target of the set wants the same name.
	pub collides: bool,
}

pub struct Resolved {
	pub renames: Vec<Rename>,
	pub findings: Vec<Finding>,
	pub unchanged: u64,
}

/// Resolve the set against the live filesystem.
pub async fn resolve(volumes: &VolumeManager, index: &VolumeIndex, set: &RenameSet) -> Resolved {
	let mut findings = Vec::new();
	let mut unchanged = 0;

	// Each target's new name, before any check.
	let mut wanted: Vec<(SdPath, PathBuf, std::fs::Metadata, String)> = Vec::new();
	let rules = match set {
		RenameSet::Ruled { rules, .. } => Some(rules.as_slice()),
		RenameSet::Named(_) => None,
	};
	let wants_capture = rules.is_some_and(|rules| {
		rules.iter().any(|rule| match rule {
			RenameRule::Replace { with, .. } => with.contains("{captured"),
			RenameRule::Affix { prefix, suffix } => {
				prefix.contains("{captured") || suffix.contains("{captured")
			}
			RenameRule::Sequence { pattern, .. } | RenameRule::Template { pattern } => {
				pattern.contains("{captured")
			}
			RenameRule::Case { .. } => false,
		})
	});
	for (position, target) in set.targets().into_iter().enumerate() {
		let Some(path) = target.as_local_path() else {
			findings.push(
				Finding::error(
					REMOTE_ROOT,
					"a file is on another device; rename it there with --device",
				)
				.at(target.clone()),
			);
			continue;
		};
		let meta = match tokio::fs::symlink_metadata(path).await {
			Ok(meta) => meta,
			Err(_) => {
				findings.push(Finding::error(MISSING, "a file is not there").at(target.clone()));
				continue;
			}
		};
		let name = path
			.file_name()
			.map(|name| name.to_string_lossy().into_owned())
			.unwrap_or_default();
		let new_name = match set {
			RenameSet::Named(pairs) => pairs[position].1.clone(),
			RenameSet::Ruled { rules, .. } => {
				let described = Target {
					name: name.clone(),
					parent: path
						.parent()
						.and_then(Path::file_name)
						.map(|name| name.to_string_lossy().into_owned())
						.unwrap_or_default(),
					index: position as u64,
					modified: meta
						.modified()
						.map(DateTime::<Local>::from)
						.unwrap_or_else(|_| Local::now()),
					captured: if wants_capture {
						captured_time(volumes, index, target).await
					} else {
						None
					},
				};
				match apply(rules, &described) {
					Ok(new_name) => new_name,
					Err(error) => {
						findings.push(Finding::error(RULE, error.to_string()).at(target.clone()));
						continue;
					}
				}
			}
		};
		if new_name == name {
			unchanged += 1;
			continue;
		}
		wanted.push((target.clone(), path.to_path_buf(), meta, new_name));
	}

	// The directories the renames happen in, each read once.
	let mut directories: HashMap<PathBuf, Directory> = HashMap::new();
	for (_, path, _, _) in &wanted {
		let Some(parent) = path.parent() else {
			continue;
		};
		if !directories.contains_key(parent) {
			let directory = Directory::read(volumes, parent).await;
			directories.insert(parent.to_path_buf(), directory);
		}
	}
	// Names the set renames away from, per directory, by fold key.
	let mut vacated: HashMap<PathBuf, HashSet<String>> = HashMap::new();
	for (_, path, _, _) in &wanted {
		if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
			let directory = &directories[parent];
			vacated
				.entry(parent.to_path_buf())
				.or_default()
				.insert(directory.fold(&name.to_string_lossy()));
		}
	}
	// Names the set claims, per directory, by fold key.
	let mut claimed: HashMap<PathBuf, HashSet<String>> = HashMap::new();

	let mut renames = Vec::with_capacity(wanted.len());
	for (target, path, meta, new_name) in wanted {
		let Some(parent) = path.parent() else {
			continue;
		};
		let directory = &directories[parent];
		let name = path
			.file_name()
			.map(|name| name.to_string_lossy().into_owned())
			.unwrap_or_default();
		let mut refused = false;
		let mut collides = false;
		let mut case_only = false;

		if let Err(problem) =
			check_portable(&new_name).and_then(|()| check_name(&new_name, directory.rules))
		{
			findings.push(Finding::error(ILLEGAL_NAME, problem.to_string()).at(target.clone()));
			refused = true;
		}

		let key = directory.fold(&new_name);
		if !claimed
			.entry(parent.to_path_buf())
			.or_default()
			.insert(key.clone())
		{
			findings.push(
				Finding::error(COLLISION, format!("two files want the name {new_name}"))
					.at(target.clone()),
			);
			refused = true;
			collides = true;
		}

		let vacated = vacated.get(parent);
		let held = directory.siblings.get(&key);
		match held {
			Some(holder) if holder == &name => {
				// Only a case-insensitive directory folds the new name onto
				// the file itself.
				case_only = true;
				findings.push(
					Finding::info(CASE_ONLY, format!("{name} changes only in case"))
						.at(target.clone()),
				);
			}
			Some(holder) if vacated.is_some_and(|names| names.contains(&key)) => {
				// The holder is renamed away by this set; the job orders it
				// first.
				let _ = holder;
			}
			Some(holder) if holder == &new_name => {
				findings.push(
					Finding::error(EXISTS, format!("a file named {new_name} is already there"))
						.at(target.clone()),
				);
				refused = true;
			}
			Some(holder) => {
				findings.push(
					Finding::error(
						CASE_COLLISION,
						format!(
							"{holder} is already there, and this directory does not tell {new_name} from it"
						),
					)
					.at(target.clone()),
				);
				refused = true;
			}
			None => {}
		}

		renames.push(Rename {
			from: path.clone(),
			to: parent.join(&new_name),
			is_dir: meta.is_dir(),
			size: if meta.is_dir() { 0 } else { meta.len() },
			case_only,
			refused,
			collides,
		});
	}

	Resolved {
		renames,
		findings,
		unchanged,
	}
}

/// One directory as the renames see it.
struct Directory {
	/// Each sibling by its fold key.
	siblings: HashMap<String, String>,
	case_insensitive: bool,
	rules: NameRules,
}

impl Directory {
	async fn read(volumes: &VolumeManager, parent: &Path) -> Self {
		let mut names = Vec::new();
		if let Ok(mut entries) = tokio::fs::read_dir(parent).await {
			while let Ok(Some(entry)) = entries.next_entry().await {
				names.push(entry.file_name().to_string_lossy().into_owned());
			}
		}
		let file_system = volumes
			.volume_for_path(parent)
			.await
			.map(|volume| volume.file_system)
			.unwrap_or_default();
		let case_insensitive = match probe_case(parent, &names).await {
			Some(answer) => answer,
			None => NameRules::case_insensitive(&file_system),
		};
		let mut siblings = HashMap::new();
		for name in names {
			let key = if case_insensitive {
				name.to_lowercase()
			} else {
				name.clone()
			};
			siblings.entry(key).or_insert(name);
		}
		Self {
			siblings,
			case_insensitive,
			rules: NameRules::of(&file_system),
		}
	}

	/// The key two names collide on in this directory.
	fn fold(&self, name: &str) -> String {
		if self.case_insensitive {
			name.to_lowercase()
		} else {
			name.to_string()
		}
	}
}

/// Whether the directory matches names without regard to case: ask for a
/// sibling under its name with the case flipped, where that spelling is not
/// itself a sibling. `None` when no sibling has a letter to flip.
async fn probe_case(parent: &Path, names: &[String]) -> Option<bool> {
	let present: HashSet<&String> = names.iter().collect();
	for name in names {
		let flipped: String = name
			.chars()
			.map(|c| {
				if c.is_lowercase() {
					c.to_uppercase().next().unwrap_or(c)
				} else if c.is_uppercase() {
					c.to_lowercase().next().unwrap_or(c)
				} else {
					c
				}
			})
			.collect();
		if flipped == *name || present.contains(&flipped) {
			continue;
		}
		return Some(
			tokio::fs::symlink_metadata(parent.join(&flipped))
				.await
				.is_ok(),
		);
	}
	None
}

/// The capture time the store holds for a file: the image's date taken or
/// the video's date captured, where the media facet has been written.
pub(crate) async fn captured_time(
	volumes: &VolumeManager,
	index: &VolumeIndex,
	target: &SdPath,
) -> Option<DateTime<Local>> {
	for reach in stores_beneath_in(volumes, index, target).await {
		if !reach.prefix.is_empty() {
			continue;
		}
		let db = index.read_store(reach.source.id).await?;
		let entry = sd_store::read::entry_by_path(db.pool(), &reach.scope)
			.await
			.ok()??;
		for (table, column) in [
			("facet_image", "date_taken"),
			("facet_video", "date_captured"),
		] {
			let sql = format!("SELECT {column} FROM {table} WHERE record_uuid = ?");
			let taken: Option<Option<String>> = sqlx::query_scalar(&sql)
				.bind(entry.uuid)
				.fetch_optional(db.pool())
				.await
				.ok()?;
			if let Some(Some(taken)) = taken {
				if let Ok(taken) = DateTime::parse_from_rfc3339(&taken) {
					return Some(taken.with_timezone(&Local));
				}
			}
		}
	}
	None
}
