use crate::{
	library::LibraryContext,
	location::indexer::IndexerError,
	prisma::{indexer_rule, PrismaClient},
};

use chrono::{DateTime, Utc};
use globset::Glob;
use int_enum::IntEnum;
use rmp_serde;
use rspc::Type;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};
use tokio::fs;

/// `IndexerRuleCreateArgs` is the argument received from the client using rspc to create a new indexer rule.
/// Note that `parameters` field **MUST** be a JSON object serialized to bytes.
///
/// In case of  `RuleKind::AcceptFilesByGlob` or `RuleKind::RejectFilesByGlob`, it will be a
/// single string containing a glob pattern.
///
/// In case of `RuleKind::AcceptIfChildrenDirectoriesArePresent` or `RuleKind::RejectIfChildrenDirectoriesArePresent` the
/// `parameters` field must be a vector of strings containing the names of the directories.
#[derive(Type, Deserialize)]
pub struct IndexerRuleCreateArgs {
	pub kind: RuleKind,
	pub name: String,
	pub parameters: Vec<u8>,
}

impl IndexerRuleCreateArgs {
	pub async fn create(self, ctx: &LibraryContext) -> Result<indexer_rule::Data, IndexerError> {
		let parameters = match self.kind {
			RuleKind::AcceptFilesByGlob | RuleKind::RejectFilesByGlob => rmp_serde::to_vec(
				&Glob::new(&serde_json::from_slice::<String>(&self.parameters)?)?,
			)?,

			RuleKind::AcceptIfChildrenDirectoriesArePresent
			| RuleKind::RejectIfChildrenDirectoriesArePresent => {
				rmp_serde::to_vec(&serde_json::from_slice::<Vec<String>>(&self.parameters)?)?
			}
		};

		ctx.db
			.indexer_rule()
			.create(self.kind as i32, self.name, parameters, vec![])
			.exec()
			.await
			.map_err(Into::into)
	}
}

#[repr(i32)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Eq, PartialEq, IntEnum, Hash)]
pub enum RuleKind {
	AcceptFilesByGlob = 0,
	RejectFilesByGlob = 1,
	AcceptIfChildrenDirectoriesArePresent = 2,
	RejectIfChildrenDirectoriesArePresent = 3,
}

/// `ParametersPerKind` is a mapping from `RuleKind` to the parameters required for each kind of rule.
/// In case of doubt about globs, consult <https://docs.rs/globset/latest/globset/#syntax>
///
/// We store directly globs in the database, serialized using rmp_serde.
///
/// In case of `ParametersPerKind::AcceptIfChildrenDirectoriesArePresent` or `ParametersPerKind::RejectIfChildrenDirectoriesArePresent`
/// first we change the data structure to a vector, then we serialize it.
#[derive(Debug)]
pub enum ParametersPerKind {
	AcceptFilesByGlob(Glob),
	RejectFilesByGlob(Glob),
	AcceptIfChildrenDirectoriesArePresent(HashSet<String>),
	RejectIfChildrenDirectoriesArePresent(HashSet<String>),
}

impl ParametersPerKind {
	async fn apply(&self, source: impl AsRef<Path>) -> Result<bool, IndexerError> {
		match self {
			ParametersPerKind::AcceptIfChildrenDirectoriesArePresent(children) => {
				accept_dir_for_its_children(source, children).await
			}
			ParametersPerKind::RejectIfChildrenDirectoriesArePresent(children) => {
				reject_dir_for_its_children(source, children).await
			}

			ParametersPerKind::AcceptFilesByGlob(glob) => accept_by_glob(source, glob),
			ParametersPerKind::RejectFilesByGlob(glob) => reject_by_glob(source, glob),
		}
	}

	fn serialize(self) -> Result<Vec<u8>, IndexerError> {
		match self {
			Self::AcceptFilesByGlob(glob) | Self::RejectFilesByGlob(glob) => {
				rmp_serde::to_vec_named(&glob).map_err(Into::into)
			}
			Self::AcceptIfChildrenDirectoriesArePresent(children)
			| Self::RejectIfChildrenDirectoriesArePresent(children) => {
				rmp_serde::to_vec(&children.into_iter().collect::<Vec<_>>()).map_err(Into::into)
			}
		}
	}
}

#[derive(Debug)]
pub struct IndexerRule {
	pub id: Option<i32>,
	pub kind: RuleKind,
	pub name: String,
	pub parameters: ParametersPerKind,
	pub date_created: DateTime<Utc>,
	pub date_modified: DateTime<Utc>,
}

impl IndexerRule {
	pub fn new(kind: RuleKind, name: String, parameters: ParametersPerKind) -> Self {
		Self {
			id: None,
			kind,
			name,
			parameters,
			date_created: Utc::now(),
			date_modified: Utc::now(),
		}
	}

	pub async fn apply(&self, source: impl AsRef<Path>) -> Result<bool, IndexerError> {
		self.parameters.apply(source).await
	}

	pub async fn save(self, client: &PrismaClient) -> Result<(), IndexerError> {
		if let Some(id) = self.id {
			client
				.indexer_rule()
				.upsert(
					indexer_rule::id::equals(id),
					(
						self.kind as i32,
						self.name,
						self.parameters.serialize()?,
						vec![],
					),
					vec![indexer_rule::date_modified::set(Utc::now().into())],
				)
				.exec()
				.await?;
		} else {
			client
				.indexer_rule()
				.create(
					self.kind as i32,
					self.name,
					self.parameters.serialize()?,
					vec![],
				)
				.exec()
				.await?;
		}

		Ok(())
	}
}

impl TryFrom<&indexer_rule::Data> for IndexerRule {
	type Error = IndexerError;

	fn try_from(data: &indexer_rule::Data) -> Result<Self, Self::Error> {
		let kind = RuleKind::from_int(data.kind)?;

		Ok(Self {
			id: Some(data.id),
			kind,
			name: data.name.clone(),
			parameters: match kind {
				RuleKind::AcceptFilesByGlob | RuleKind::RejectFilesByGlob => {
					let glob_str = rmp_serde::from_slice(&data.parameters)?;
					if matches!(kind, RuleKind::AcceptFilesByGlob) {
						ParametersPerKind::AcceptFilesByGlob(glob_str)
					} else {
						ParametersPerKind::RejectFilesByGlob(glob_str)
					}
				}
				RuleKind::AcceptIfChildrenDirectoriesArePresent
				| RuleKind::RejectIfChildrenDirectoriesArePresent => {
					let childrens = rmp_serde::from_slice::<Vec<String>>(&data.parameters)?
						.into_iter()
						.collect();
					if matches!(kind, RuleKind::AcceptIfChildrenDirectoriesArePresent) {
						ParametersPerKind::AcceptIfChildrenDirectoriesArePresent(childrens)
					} else {
						ParametersPerKind::RejectIfChildrenDirectoriesArePresent(childrens)
					}
				}
			},
			date_created: data.date_created.into(),
			date_modified: data.date_modified.into(),
		})
	}
}

impl TryFrom<indexer_rule::Data> for IndexerRule {
	type Error = IndexerError;

	fn try_from(data: indexer_rule::Data) -> Result<Self, Self::Error> {
		Self::try_from(&data)
	}
}

fn accept_by_glob(source: impl AsRef<Path>, glob: &Glob) -> Result<bool, IndexerError> {
	Ok(glob.compile_matcher().is_match(source.as_ref()))
}

fn reject_by_glob(source: impl AsRef<Path>, reject_glob: &Glob) -> Result<bool, IndexerError> {
	Ok(!reject_glob.compile_matcher().is_match(source.as_ref()))
}

async fn accept_dir_for_its_children(
	source: impl AsRef<Path>,
	children: &HashSet<String>,
) -> Result<bool, IndexerError> {
	let source = source.as_ref();
	let mut read_dir = fs::read_dir(source).await?;
	while let Some(entry) = read_dir.next_entry().await? {
		if entry.metadata().await?.is_dir()
			&& children.contains(entry.file_name().to_string_lossy().as_ref())
		{
			return Ok(true);
		}
	}

	Ok(false)
}

async fn reject_dir_for_its_children(
	source: impl AsRef<Path>,
	children: &HashSet<String>,
) -> Result<bool, IndexerError> {
	let source = source.as_ref();
	let mut read_dir = fs::read_dir(source).await?;
	while let Some(entry) = read_dir.next_entry().await? {
		if entry.metadata().await?.is_dir()
			&& children.contains(entry.file_name().to_string_lossy().as_ref())
		{
			return Ok(false);
		}
	}

	Ok(true)
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::tempdir;
	use tokio::fs;

	#[tokio::test]
	async fn test_reject_hidden_file() {
		let hidden = Path::new(".hidden.txt");
		let normal = Path::new("normal.txt");
		let hidden_inner_dir = Path::new("/test/.hidden/");
		let hidden_inner_file = Path::new("/test/.hidden/file.txt");
		let normal_inner_dir = Path::new("/test/normal/");
		let normal_inner_file = Path::new("/test/normal/inner.txt");
		let rule = IndexerRule::new(
			RuleKind::RejectFilesByGlob,
			"ignore hidden files".to_string(),
			ParametersPerKind::RejectFilesByGlob(Glob::new("**/.*").unwrap()),
		);
		assert!(!rule.apply(hidden).await.unwrap());
		assert!(rule.apply(normal).await.unwrap());
		assert!(!rule.apply(hidden_inner_dir).await.unwrap());
		assert!(!rule.apply(hidden_inner_file).await.unwrap());
		assert!(rule.apply(normal_inner_dir).await.unwrap());
		assert!(rule.apply(normal_inner_file).await.unwrap());
	}

	#[tokio::test]
	async fn test_reject_specific_dir() {
		let project_file = Path::new("/test/project/src/main.rs");
		let project_build_dir = Path::new("/test/project/target");
		let project_build_dir_inner = Path::new("/test/project/target/debug/");

		let rule = IndexerRule::new(
			RuleKind::RejectFilesByGlob,
			"ignore build directory".to_string(),
			ParametersPerKind::RejectFilesByGlob(Glob::new("{**/target/*,**/target}").unwrap()),
		);

		assert!(rule.apply(project_file).await.unwrap());
		assert!(!rule.apply(project_build_dir).await.unwrap());
		assert!(!rule.apply(project_build_dir_inner).await.unwrap());
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
			RuleKind::AcceptFilesByGlob,
			"only photos".to_string(),
			ParametersPerKind::AcceptFilesByGlob(Glob::new("*.{jpg,png,jpeg}").unwrap()),
		);
		assert!(!rule.apply(text).await.unwrap());
		assert!(rule.apply(png).await.unwrap());
		assert!(rule.apply(jpg).await.unwrap());
		assert!(rule.apply(jpeg).await.unwrap());
		assert!(!rule.apply(inner_text).await.unwrap());
		assert!(rule.apply(inner_png).await.unwrap());
		assert!(rule.apply(inner_jpg).await.unwrap());
		assert!(rule.apply(inner_jpeg).await.unwrap());
		assert!(!rule.apply(many_inner_dirs_text).await.unwrap());
		assert!(rule.apply(many_inner_dirs_png).await.unwrap());
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
			RuleKind::AcceptIfChildrenDirectoriesArePresent,
			"git projects".to_string(),
			ParametersPerKind::AcceptIfChildrenDirectoriesArePresent(childrens),
		);

		assert!(rule.apply(project1).await.unwrap());
		assert!(rule.apply(project2).await.unwrap());
		assert!(!rule.apply(not_project).await.unwrap());
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
			RuleKind::RejectIfChildrenDirectoriesArePresent,
			"git projects".to_string(),
			ParametersPerKind::RejectIfChildrenDirectoriesArePresent(childrens),
		);

		assert!(!rule.apply(project1).await.unwrap());
		assert!(!rule.apply(project2).await.unwrap());
		assert!(rule.apply(not_project).await.unwrap());
	}
}
