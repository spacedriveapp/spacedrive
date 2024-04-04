use std::{collections::HashMap, path::Path};

use chrono::{DateTime, Utc};
use futures_util::future::try_join_all;
use sd_prisma::prisma::indexer_rule;
use sd_utils::db::maybe_missing;
use serde::{Deserialize, Serialize};

use super::{IndexerRuleError, RuleKind, RulePerKind};

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexerRule {
	pub id: Option<i32>,
	pub name: String,
	pub default: bool,
	pub rules: Vec<RulePerKind>,
	pub date_created: DateTime<Utc>,
	pub date_modified: DateTime<Utc>,
}

impl IndexerRule {
	pub async fn apply(
		&self,
		source: impl AsRef<Path>,
	) -> Result<Vec<(RuleKind, bool)>, IndexerRuleError> {
		try_join_all(self.rules.iter().map(|rule| rule.apply(source.as_ref()))).await
	}

	pub async fn apply_all(
		rules: &[IndexerRule],
		source: impl AsRef<Path>,
	) -> Result<HashMap<RuleKind, Vec<bool>>, IndexerRuleError> {
		try_join_all(rules.iter().map(|rule| rule.apply(source.as_ref())))
			.await
			.map(|results| {
				results.into_iter().flatten().fold(
					HashMap::<_, Vec<_>>::with_capacity(RuleKind::variant_count()),
					|mut map, (kind, result)| {
						map.entry(kind).or_default().push(result);
						map
					},
				)
			})
	}
}

impl TryFrom<&indexer_rule::Data> for IndexerRule {
	type Error = IndexerRuleError;

	fn try_from(data: &indexer_rule::Data) -> Result<Self, Self::Error> {
		Ok(Self {
			id: Some(data.id),
			name: maybe_missing(data.name.clone(), "indexer_rule.name")?,
			default: data.default.unwrap_or_default(),
			rules: rmp_serde::from_slice(maybe_missing(
				&data.rules_per_kind,
				"indexer_rule.rules_per_kind",
			)?)?,
			date_created: maybe_missing(data.date_created, "indexer_rule.date_created")?.into(),
			date_modified: maybe_missing(data.date_modified, "indexer_rule.date_modified")?.into(),
		})
	}
}

impl TryFrom<indexer_rule::Data> for IndexerRule {
	type Error = IndexerRuleError;

	fn try_from(data: indexer_rule::Data) -> Result<Self, Self::Error> {
		Self::try_from(&data)
	}
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
	use std::collections::HashSet;

	use super::*;
	use globset::{Glob, GlobSetBuilder};
	use tempfile::tempdir;
	use tokio::fs;

	impl IndexerRule {
		pub fn new(name: String, default: bool, rules: Vec<RulePerKind>) -> Self {
			Self {
				id: None,
				name,
				default,
				rules,
				date_created: Utc::now(),
				date_modified: Utc::now(),
			}
		}
	}

	async fn check_rule(indexer_rule: &IndexerRule, path: impl AsRef<Path>) -> bool {
		indexer_rule
			.apply(path)
			.await
			.unwrap()
			.into_iter()
			.all(|(_kind, res)| res)
	}

	#[tokio::test]
	async fn test_reject_hidden_file() {
		let hidden = Path::new(".hidden.txt");
		let normal = Path::new("normal.txt");
		let hidden_inner_dir = Path::new("/test/.hidden/");
		let hidden_inner_file = Path::new("/test/.hidden/file.txt");
		let normal_inner_dir = Path::new("/test/normal/");
		let normal_inner_file = Path::new("/test/normal/inner.txt");
		let rule = IndexerRule::new(
			"ignore hidden files".to_string(),
			false,
			vec![RulePerKind::RejectFilesByGlob(
				vec![],
				GlobSetBuilder::new()
					.add(Glob::new("**/.*").unwrap())
					.build()
					.unwrap(),
			)],
		);

		assert!(!check_rule(&rule, hidden).await);
		assert!(check_rule(&rule, normal).await);
		assert!(!check_rule(&rule, hidden_inner_dir).await);
		assert!(!check_rule(&rule, hidden_inner_file).await);
		assert!(check_rule(&rule, normal_inner_dir).await);
		assert!(check_rule(&rule, normal_inner_file).await);
	}

	#[tokio::test]
	async fn test_reject_specific_dir() {
		let project_file = Path::new("/test/project/src/main.rs");
		let project_build_dir = Path::new("/test/project/target");
		let project_build_dir_inner = Path::new("/test/project/target/debug/");

		let rule = IndexerRule::new(
			"ignore build directory".to_string(),
			false,
			vec![RulePerKind::RejectFilesByGlob(
				vec![],
				GlobSetBuilder::new()
					.add(Glob::new("{**/target/*,**/target}").unwrap())
					.build()
					.unwrap(),
			)],
		);

		assert!(check_rule(&rule, project_file).await);
		assert!(!check_rule(&rule, project_build_dir).await);
		assert!(!check_rule(&rule, project_build_dir_inner).await);
	}

	#[tokio::test]
	async fn test_only_photos() {
		let text = Path::new("file.txt");
		let png = Path::new("photo1.png");
		let jpg = Path::new("photo1.png");
		let jpeg = Path::new("photo3.jpeg");
		let inner_text = Path::new("/test/file.txt");
		let inner_png = Path::new("/test/photo1.png");
		let inner_jpg = Path::new("/test/photo2.jpg");
		let inner_jpeg = Path::new("/test/photo3.jpeg");
		let many_inner_dirs_text = Path::new("/test/1/2/3/4/4/5/6/file.txt");
		let many_inner_dirs_png = Path::new("/test/1/2/3/4/4/5/6/photo1.png");
		let rule = IndexerRule::new(
			"only photos".to_string(),
			false,
			vec![RulePerKind::AcceptFilesByGlob(
				vec![],
				GlobSetBuilder::new()
					.add(Glob::new("*.{jpg,png,jpeg}").unwrap())
					.build()
					.unwrap(),
			)],
		);

		assert!(!check_rule(&rule, text).await);
		assert!(check_rule(&rule, png).await);
		assert!(check_rule(&rule, jpg).await);
		assert!(check_rule(&rule, jpeg).await);
		assert!(!check_rule(&rule, inner_text).await);
		assert!(check_rule(&rule, inner_png).await);
		assert!(check_rule(&rule, inner_jpg).await);
		assert!(check_rule(&rule, inner_jpeg).await);
		assert!(!check_rule(&rule, many_inner_dirs_text).await);
		assert!(check_rule(&rule, many_inner_dirs_png).await);
	}

	#[tokio::test]
	async fn test_directory_has_children() {
		let root = tempdir().unwrap();

		let project1 = root.path().join("project1");
		let project2 = root.path().join("project2");
		let not_project = root.path().join("not_project");

		fs::create_dir(&project1).await.unwrap();
		fs::create_dir(&project2).await.unwrap();
		fs::create_dir(&not_project).await.unwrap();

		fs::create_dir(project1.join(".git")).await.unwrap();
		fs::create_dir(project2.join(".git")).await.unwrap();
		fs::create_dir(project2.join("books")).await.unwrap();

		let childrens = [".git".to_string()].into_iter().collect::<HashSet<_>>();

		let rule = IndexerRule::new(
			"git projects".to_string(),
			false,
			vec![RulePerKind::AcceptIfChildrenDirectoriesArePresent(
				childrens,
			)],
		);

		assert!(check_rule(&rule, project1).await);
		assert!(check_rule(&rule, project2).await);
		assert!(!check_rule(&rule, not_project).await);
	}

	#[tokio::test]
	async fn test_reject_directory_by_its_children() {
		let root = tempdir().unwrap();

		let project1 = root.path().join("project1");
		let project2 = root.path().join("project2");
		let not_project = root.path().join("not_project");

		fs::create_dir(&project1).await.unwrap();
		fs::create_dir(&project2).await.unwrap();
		fs::create_dir(&not_project).await.unwrap();

		fs::create_dir(project1.join(".git")).await.unwrap();
		fs::create_dir(project2.join(".git")).await.unwrap();
		fs::create_dir(project2.join("books")).await.unwrap();

		let childrens = [".git".to_string()].into_iter().collect::<HashSet<_>>();

		let rule = IndexerRule::new(
			"git projects".to_string(),
			false,
			vec![RulePerKind::RejectIfChildrenDirectoriesArePresent(
				childrens,
			)],
		);

		assert!(!check_rule(&rule, project1).await);
		assert!(!check_rule(&rule, project2).await);
		assert!(check_rule(&rule, not_project).await);
	}

	impl PartialEq for RulePerKind {
		fn eq(&self, other: &Self) -> bool {
			match (self, other) {
				(
					RulePerKind::AcceptFilesByGlob(self_globs, _),
					RulePerKind::AcceptFilesByGlob(other_globs, _),
				) => self_globs == other_globs,
				(
					RulePerKind::RejectFilesByGlob(self_globs, _),
					RulePerKind::RejectFilesByGlob(other_globs, _),
				) => self_globs == other_globs,
				(
					RulePerKind::AcceptIfChildrenDirectoriesArePresent(self_childrens),
					RulePerKind::AcceptIfChildrenDirectoriesArePresent(other_childrens),
				) => self_childrens == other_childrens,
				(
					RulePerKind::RejectIfChildrenDirectoriesArePresent(self_childrens),
					RulePerKind::RejectIfChildrenDirectoriesArePresent(other_childrens),
				) => self_childrens == other_childrens,
				_ => false,
			}
		}
	}

	impl Eq for RulePerKind {}

	impl PartialEq for IndexerRule {
		fn eq(&self, other: &Self) -> bool {
			self.id == other.id
				&& self.name == other.name
				&& self.default == other.default
				&& self.rules == other.rules
				&& self.date_created == other.date_created
				&& self.date_modified == other.date_modified
		}
	}

	impl Eq for IndexerRule {}

	#[test]
	fn serde_smoke_test() {
		let actual = IndexerRule::new(
			"No Hidden".to_string(),
			true,
			vec![RulePerKind::RejectFilesByGlob(
				vec![Glob::new("**/.*").unwrap()],
				Glob::new("**/.*")
					.and_then(|glob| GlobSetBuilder::new().add(glob).build())
					.unwrap(),
			)],
		);

		let expected =
			rmp_serde::from_slice::<IndexerRule>(&rmp_serde::to_vec_named(&actual).unwrap())
				.unwrap();

		assert_eq!(actual, expected);
	}
}
