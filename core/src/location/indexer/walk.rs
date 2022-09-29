use chrono::{DateTime, Utc};
use std::{
	cmp::Ordering,
	collections::{HashMap, VecDeque},
	hash::{Hash, Hasher},
	path::{Path, PathBuf},
};
use tokio::fs;
use tracing::{debug, error};

use super::{
	rules::{IndexerRule, RuleKind},
	IndexerError,
};

/// `WalkEntry` represents a single path in the filesystem, for any comparison purposes, we only
/// consider the path itself, not the metadata.
#[derive(Clone, Debug)]
pub(super) struct WalkEntry {
	pub(super) path: PathBuf,
	pub(super) is_dir: bool,
	pub(super) created_at: DateTime<Utc>,
}

impl PartialEq for WalkEntry {
	fn eq(&self, other: &Self) -> bool {
		self.path == other.path
	}
}

impl Eq for WalkEntry {}

impl Hash for WalkEntry {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.path.hash(state);
	}
}

impl PartialOrd for WalkEntry {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		self.path.partial_cmp(&other.path)
	}
}

impl Ord for WalkEntry {
	fn cmp(&self, other: &Self) -> Ordering {
		self.path.cmp(&other.path)
	}
}

/// This function walks through the filesystem, applying the rules to each entry and then returning
/// a list of accepted entries. There are some useful comments in the implementation of this function
/// in case of doubts.
pub(super) async fn walk(
	root: PathBuf,
	rules_per_kind: &HashMap<RuleKind, Vec<IndexerRule>>,
	update_notifier: impl Fn(&Path, usize),
) -> Result<Vec<WalkEntry>, IndexerError> {
	let mut to_walk = VecDeque::with_capacity(1);
	to_walk.push_back((root.clone(), None));
	let mut indexed_paths = HashMap::new();

	while let Some((current_path, parent_dir_accepted_by_its_children)) = to_walk.pop_front() {
		let mut read_dir = match fs::read_dir(&current_path).await {
			Ok(read_dir) => read_dir,
			Err(e) => {
				error!(
					"Error reading directory {}: {:#?}",
					current_path.display(),
					e
				);
				continue;
			}
		};

		// Marking with a loop label here in case of rejection or erros, to continue with next entry
		'entries: loop {
			let entry = match read_dir.next_entry().await {
				Ok(Some(entry)) => entry,
				Ok(None) => break,
				Err(e) => {
					error!(
						"Error reading entry in {}: {:#?}",
						current_path.display(),
						e
					);
					continue;
				}
			};

			// Accept by children has three states,
			// None if we don't now yet or if this check doesn't apply
			// Some(true) if this check applies and it passes
			// Some(false) if this check applies and it was rejected
			// and we pass the current parent state to its children
			let mut accept_by_children_dir = parent_dir_accepted_by_its_children;

			let current_path = entry.path();

			update_notifier(&current_path, indexed_paths.len());

			debug!(
				"Current filesystem path: {}, accept_by_children_dir: {:#?}",
				current_path.display(),
				accept_by_children_dir
			);
			if let Some(reject_rules) = rules_per_kind.get(&RuleKind::RejectFilesByGlob) {
				for reject_rule in reject_rules {
					// It's ok to unwrap here, reject rules are infallible
					if !reject_rule.apply(&current_path).await.unwrap() {
						debug!(
							"Path {} rejected by rule {}",
							current_path.display(),
							reject_rule.name
						);
						continue 'entries;
					}
				}
			}

			let metadata = entry.metadata().await?;

			// TODO: Hard ignoring symlinks for now, but this should be configurable
			if metadata.is_symlink() {
				continue 'entries;
			}

			let is_dir = metadata.is_dir();

			if is_dir {
				// If it is a directory, first we check if we must reject it and its children entirely
				if let Some(reject_by_children_rules) =
					rules_per_kind.get(&RuleKind::RejectIfChildrenDirectoriesArePresent)
				{
					for reject_by_children_rule in reject_by_children_rules {
						match reject_by_children_rule.apply(&current_path).await {
							Ok(false) => {
								debug!(
									"Path {} rejected by rule {}",
									current_path.display(),
									reject_by_children_rule.name
								);
								continue 'entries;
							}
							Ok(true) => {}
							Err(e) => {
								error!(
									"Error applying rule {} to path {}: {:#?}",
									reject_by_children_rule.name,
									current_path.display(),
									e
								);
								continue 'entries;
							}
						}
					}
				}

				// Then we check if we must accept it and its children
				if let Some(accept_by_children_rules) =
					rules_per_kind.get(&RuleKind::AcceptIfChildrenDirectoriesArePresent)
				{
					for accept_by_children_rule in accept_by_children_rules {
						match accept_by_children_rule.apply(&current_path).await {
							Ok(true) => {
								accept_by_children_dir = Some(true);
								break;
							}
							Ok(false) => {}
							Err(e) => {
								error!(
									"Error applying rule {} to path {}: {:#?}",
									accept_by_children_rule.name,
									current_path.display(),
									e
								);
								continue 'entries;
							}
						}
					}

					// If it wasn't accepted then we mark as rejected
					if accept_by_children_dir.is_none() {
						debug!(
							"Path {} rejected because it didn't passed in any AcceptIfChildrenDirectoriesArePresent rule",
							current_path.display()
						);
						accept_by_children_dir = Some(false);
					}
				}

				// Then we mark this directory the be walked in too
				to_walk.push_back((entry.path(), accept_by_children_dir));
			}

			let mut accept_by_glob = false;
			if let Some(accept_rules) = rules_per_kind.get(&RuleKind::AcceptFilesByGlob) {
				for accept_rule in accept_rules {
					// It's ok to unwrap here, accept rules are infallible
					if accept_rule.apply(&current_path).await.unwrap() {
						debug!(
							"Path {} accepted by rule {}",
							current_path.display(),
							accept_rule.name
						);
						accept_by_glob = true;
						break;
					}
				}
				if !accept_by_glob {
					debug!(
						"Path {} reject because it didn't passed in any AcceptFilesByGlob rules",
						current_path.display()
					);
					continue 'entries;
				}
			} else {
				// If there are no accept rules, then accept all paths
				accept_by_glob = true;
			}

			if accept_by_glob
				&& (accept_by_children_dir.is_none() || accept_by_children_dir.unwrap())
			{
				indexed_paths.insert(
					current_path.clone(),
					WalkEntry {
						path: current_path.clone(),
						is_dir,
						created_at: metadata.created()?.into(),
					},
				);

				// If the ancestors directories wasn't indexed before, now we do
				for ancestor in current_path
					.ancestors()
					.skip(1) // Skip the current directory as it was already indexed
					.take_while(|&ancestor| ancestor != root)
				{
					debug!("Indexing ancestor {}", ancestor.display());
					if !indexed_paths.contains_key(ancestor) {
						indexed_paths.insert(
							ancestor.to_path_buf(),
							WalkEntry {
								path: ancestor.to_path_buf(),
								is_dir: true,
								created_at: fs::metadata(ancestor).await?.created()?.into(),
							},
						);
					} else {
						// If indexed_paths contains the current ancestors, then it will contain
						// also all if its ancestors too, so we can stop here
						break;
					}
				}
			}
		}
	}

	let mut indexed_paths = indexed_paths.into_values().collect::<Vec<_>>();
	// Also adding the root location path
	let root_created_at = fs::metadata(&root).await?.created()?.into();
	indexed_paths.push(WalkEntry {
		path: root,
		is_dir: true,
		created_at: root_created_at,
	});
	// Sorting so we can give each path a crescent id given the filesystem hierarchy
	indexed_paths.sort();

	Ok(indexed_paths)
}

#[cfg(test)]
mod tests {
	use super::super::rules::ParametersPerKind;
	use super::*;
	use chrono::Utc;
	use globset::Glob;
	use std::collections::BTreeSet;
	use tempfile::{tempdir, TempDir};
	use tokio::fs;
	use tracing_test::traced_test;

	async fn prepare_location() -> TempDir {
		let root = tempdir().unwrap();
		let root_path = root.path();
		let rust_project = root_path.join("rust_project");
		let inner_project = root_path.join("inner");
		let node_project = inner_project.join("node_project");
		let photos = root_path.join("photos");

		fs::create_dir(&rust_project).await.unwrap();
		fs::create_dir(&inner_project).await.unwrap();
		fs::create_dir(&node_project).await.unwrap();
		fs::create_dir(&photos).await.unwrap();

		// Making rust and node projects a git repository
		fs::create_dir(rust_project.join(".git")).await.unwrap();
		fs::create_dir(node_project.join(".git")).await.unwrap();

		// Populating rust project
		fs::File::create(rust_project.join("Cargo.toml"))
			.await
			.unwrap();
		let rust_src_dir = rust_project.join("src");
		fs::create_dir(&rust_src_dir).await.unwrap();
		fs::File::create(rust_src_dir.join("main.rs"))
			.await
			.unwrap();
		let rust_target_dir = rust_project.join("target");
		fs::create_dir(&rust_target_dir).await.unwrap();
		let rust_build_dir = rust_target_dir.join("debug");
		fs::create_dir(&rust_build_dir).await.unwrap();
		fs::File::create(rust_build_dir.join("main")).await.unwrap();

		// Populating node project
		fs::File::create(node_project.join("package.json"))
			.await
			.unwrap();
		let node_src_dir = node_project.join("src");
		fs::create_dir(&node_src_dir).await.unwrap();
		fs::File::create(node_src_dir.join("App.tsx"))
			.await
			.unwrap();
		let node_modules = node_project.join("node_modules");
		fs::create_dir(&node_modules).await.unwrap();
		let node_modules_dep = node_modules.join("react");
		fs::create_dir(&node_modules_dep).await.unwrap();
		fs::File::create(node_modules_dep.join("package.json"))
			.await
			.unwrap();

		// Photos directory
		for photo in ["photo1.png", "photo2.jpg", "photo3.jpeg", "text.txt"].iter() {
			fs::File::create(photos.join(photo)).await.unwrap();
		}

		root
	}

	#[tokio::test]
	async fn test_walk_without_rules() {
		let root = prepare_location().await;
		let root_path = root.path();

		let any_datetime = Utc::now();

		#[rustfmt::skip]
		let expected = [
			WalkEntry { path: root_path.to_path_buf(), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project/.git"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project/Cargo.toml"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project/src"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project/src/main.rs"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project/target"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project/target/debug"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project/target/debug/main"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project/.git"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project/package.json"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project/src"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project/src/App.tsx"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project/node_modules"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project/node_modules/react"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project/node_modules/react/package.json"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("photos"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("photos/photo1.png"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("photos/photo2.jpg"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("photos/photo3.jpeg"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("photos/text.txt"), is_dir: false, created_at: any_datetime },
		]
		.into_iter()
		.collect::<BTreeSet<_>>();

		let actual = walk(root_path.to_path_buf(), &HashMap::new(), |_, _| {})
			.await
			.unwrap()
			.into_iter()
			.collect::<BTreeSet<_>>();

		assert_eq!(actual, expected);
	}

	#[tokio::test]
	#[traced_test]
	async fn test_only_photos() {
		let root = prepare_location().await;
		let root_path = root.path();

		let any_datetime = Utc::now();

		#[rustfmt::skip]
		let expected = [
			WalkEntry { path: root_path.to_path_buf(), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("photos"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("photos/photo1.png"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("photos/photo2.jpg"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("photos/photo3.jpeg"), is_dir: false, created_at: any_datetime },
		]
		.into_iter()
		.collect::<BTreeSet<_>>();

		let only_photos_rule = [(
			RuleKind::AcceptFilesByGlob,
			vec![IndexerRule::new(
				RuleKind::AcceptFilesByGlob,
				"only photos".to_string(),
				ParametersPerKind::AcceptFilesByGlob(Glob::new("{*.png,*.jpg,*.jpeg}").unwrap()),
			)],
		)]
		.into_iter()
		.collect::<HashMap<_, _>>();

		let actual = walk(root_path.to_path_buf(), &only_photos_rule, |_, _| {})
			.await
			.unwrap()
			.into_iter()
			.collect::<BTreeSet<_>>();

		assert_eq!(actual, expected);
	}

	#[tokio::test]
	#[traced_test]
	async fn test_git_repos() {
		let root = prepare_location().await;
		let root_path = root.path();

		let any_datetime = Utc::now();

		#[rustfmt::skip]
		let expected = [
			WalkEntry { path: root_path.to_path_buf(), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project/.git"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project/Cargo.toml"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project/src"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project/src/main.rs"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project/target"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project/target/debug"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project/target/debug/main"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project/.git"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project/package.json"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project/src"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project/src/App.tsx"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project/node_modules"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project/node_modules/react"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project/node_modules/react/package.json"), is_dir: false, created_at: any_datetime },
		]
		.into_iter()
		.collect::<BTreeSet<_>>();

		let git_repos = [(
			RuleKind::AcceptIfChildrenDirectoriesArePresent,
			vec![IndexerRule::new(
				RuleKind::AcceptIfChildrenDirectoriesArePresent,
				"git repos".to_string(),
				ParametersPerKind::AcceptIfChildrenDirectoriesArePresent(
					[".git".to_string()].into_iter().collect(),
				),
			)],
		)]
		.into_iter()
		.collect::<HashMap<_, _>>();

		let actual = walk(root_path.to_path_buf(), &git_repos, |_, _| {})
			.await
			.unwrap()
			.into_iter()
			.collect::<BTreeSet<_>>();

		assert_eq!(actual, expected);
	}

	#[tokio::test]
	#[traced_test]
	async fn git_repos_without_deps_or_build_dirs() {
		let root = prepare_location().await;
		let root_path = root.path();

		let any_datetime = Utc::now();

		#[rustfmt::skip]
		let expected = [
			WalkEntry { path: root_path.to_path_buf(), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project/.git"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project/Cargo.toml"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project/src"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("rust_project/src/main.rs"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project/.git"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project/package.json"), is_dir: false, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project/src"), is_dir: true, created_at: any_datetime.clone() },
			WalkEntry { path: root_path.join("inner/node_project/src/App.tsx"), is_dir: false, created_at: any_datetime },
		]
		.into_iter()
		.collect::<BTreeSet<_>>();

		let git_repos_no_deps_no_build_dirs = [
			(
				RuleKind::AcceptIfChildrenDirectoriesArePresent,
				vec![IndexerRule::new(
					RuleKind::AcceptIfChildrenDirectoriesArePresent,
					"git repos".to_string(),
					ParametersPerKind::AcceptIfChildrenDirectoriesArePresent(
						[".git".to_string()].into_iter().collect(),
					),
				)],
			),
			(
				RuleKind::RejectFilesByGlob,
				vec![
					IndexerRule::new(
						RuleKind::RejectFilesByGlob,
						"reject node_modules".to_string(),
						ParametersPerKind::RejectFilesByGlob(
							Glob::new("{**/node_modules/*,**/node_modules}").unwrap(),
						),
					),
					IndexerRule::new(
						RuleKind::RejectFilesByGlob,
						"reject rust build dir".to_string(),
						ParametersPerKind::RejectFilesByGlob(
							Glob::new("{**/target/*,**/target}").unwrap(),
						),
					),
				],
			),
		]
		.into_iter()
		.collect::<HashMap<_, _>>();

		let actual = walk(
			root_path.to_path_buf(),
			&git_repos_no_deps_no_build_dirs,
			|_, _| {},
		)
		.await
		.unwrap()
		.into_iter()
		.collect::<BTreeSet<_>>();

		assert_eq!(actual, expected);
	}
}
