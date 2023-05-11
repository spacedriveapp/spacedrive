use crate::{
	library::Library,
	location::location_with_indexer_rules,
	prisma::{indexer_rule, PrismaClient},
	util::error::{FileIOError, NonUtf8PathError},
};

use chrono::{DateTime, Utc};
use globset::{Glob, GlobSet, GlobSetBuilder};
use rmp_serde::{self, decode, encode};
use rspc::ErrorCode;
use serde::{de, ser, Deserialize, Serialize};
use specta::Type;
use std::{
	collections::{HashMap, HashSet},
	marker::PhantomData,
	path::Path,
};
use thiserror::Error;
use tokio::fs;
use tracing::debug;

#[derive(Error, Debug)]
pub enum IndexerRuleError {
	// User errors
	#[error("invalid indexer rule kind integer: {0}")]
	InvalidRuleKindInt(i32),
	#[error("glob builder error")]
	Glob(#[from] globset::Error),
	#[error(transparent)]
	NonUtf8Path(#[from] NonUtf8PathError),

	// Internal Errors
	#[error("indexer rule parameters encode error")]
	RuleParametersRMPEncode(#[from] encode::Error),
	#[error("indexer rule parameters decode error")]
	RuleParametersRMPDecode(#[from] decode::Error),
	#[error("accept by its children file I/O error")]
	AcceptByItsChildrenFileIO(FileIOError),
	#[error("reject by its children file I/O error")]
	RejectByItsChildrenFileIO(FileIOError),
	#[error("database error")]
	Database(#[from] prisma_client_rust::QueryError),
}

impl From<IndexerRuleError> for rspc::Error {
	fn from(err: IndexerRuleError) -> Self {
		match err {
			IndexerRuleError::InvalidRuleKindInt(_)
			| IndexerRuleError::Glob(_)
			| IndexerRuleError::NonUtf8Path(_) => {
				rspc::Error::with_cause(ErrorCode::BadRequest, err.to_string(), err)
			}

			_ => rspc::Error::with_cause(ErrorCode::InternalServerError, err.to_string(), err),
		}
	}
}

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
	pub dry_run: bool,
	pub parameters: Vec<String>,
}

impl IndexerRuleCreateArgs {
	pub async fn create(
		self,
		library: &Library,
	) -> Result<Option<indexer_rule::Data>, IndexerRuleError> {
		debug!(
			"{} a new indexer rule (name = {}, params = {:?})",
			if self.dry_run {
				"Dry run: Would create"
			} else {
				"Trying to create"
			},
			self.name,
			self.parameters
		);

		let parameters = match self.kind {
			RuleKind::AcceptFilesByGlob | RuleKind::RejectFilesByGlob => rmp_serde::to_vec(
				&self
					.parameters
					.into_iter()
					.map(|s| Glob::new(s.as_str()))
					.collect::<Result<Vec<Glob>, _>>()?,
			)?,

			RuleKind::AcceptIfChildrenDirectoriesArePresent
			| RuleKind::RejectIfChildrenDirectoriesArePresent => rmp_serde::to_vec(&self.parameters)?,
		};

		if self.dry_run {
			return Ok(None);
		}

		Ok(Some(
			library
				.db
				.indexer_rule()
				.create(self.kind as i32, self.name, parameters, vec![])
				.exec()
				.await?,
		))
	}
}

#[repr(i32)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Eq, PartialEq, Hash)]
pub enum RuleKind {
	AcceptFilesByGlob = 0,
	RejectFilesByGlob = 1,
	AcceptIfChildrenDirectoriesArePresent = 2,
	RejectIfChildrenDirectoriesArePresent = 3,
}

impl RuleKind {
	pub const fn variant_count() -> usize {
		// TODO: Use https://doc.rust-lang.org/std/mem/fn.variant_count.html if it ever gets stabilized
		4
	}
}

impl TryFrom<i32> for RuleKind {
	type Error = IndexerRuleError;

	fn try_from(value: i32) -> Result<Self, Self::Error> {
		let s = match value {
			0 => Self::AcceptFilesByGlob,
			1 => Self::RejectFilesByGlob,
			2 => Self::AcceptIfChildrenDirectoriesArePresent,
			3 => Self::RejectIfChildrenDirectoriesArePresent,
			_ => return Err(Self::Error::InvalidRuleKindInt(value)),
		};

		Ok(s)
	}
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
	// TODO: Add an indexer rule that filter files based on their extended attributes
	// https://learn.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants
	// https://en.wikipedia.org/wiki/Extended_file_attributes
	AcceptFilesByGlob(Vec<Glob>, GlobSet),
	RejectFilesByGlob(Vec<Glob>, GlobSet),
	AcceptIfChildrenDirectoriesArePresent(HashSet<String>),
	RejectIfChildrenDirectoriesArePresent(HashSet<String>),
}

impl ParametersPerKind {
	fn new_files_by_globs_str_and_kind(
		globs_str: impl IntoIterator<Item = impl AsRef<str>>,
		kind_fn: impl Fn(Vec<Glob>, GlobSet) -> Self,
	) -> Result<Self, IndexerRuleError> {
		globs_str
			.into_iter()
			.map(|s| s.as_ref().parse::<Glob>())
			.collect::<Result<Vec<_>, _>>()
			.and_then(|globs| {
				globs
					.iter()
					.cloned()
					.fold(&mut GlobSetBuilder::new(), |builder, glob| {
						builder.add(glob)
					})
					.build()
					.map(move |glob_set| kind_fn(globs, glob_set))
					.map_err(Into::into)
			})
			.map_err(Into::into)
	}

	pub fn new_accept_files_by_globs_str(
		globs_str: impl IntoIterator<Item = impl AsRef<str>>,
	) -> Result<Self, IndexerRuleError> {
		Self::new_files_by_globs_str_and_kind(globs_str, Self::AcceptFilesByGlob)
	}

	pub fn new_reject_files_by_glob(
		globs_str: impl IntoIterator<Item = impl AsRef<str>>,
	) -> Result<Self, IndexerRuleError> {
		Self::new_files_by_globs_str_and_kind(globs_str, Self::RejectFilesByGlob)
	}
}

/// We're implementing `Serialize` by hand as `GlobSet`s aren't serializable, so we ignore them on
/// serialization
impl Serialize for ParametersPerKind {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: ser::Serializer,
	{
		match *self {
			ParametersPerKind::AcceptFilesByGlob(ref globs, ref _glob_set) => serializer
				.serialize_newtype_variant("ParametersPerKind", 0, "AcceptFilesByGlob", globs),
			ParametersPerKind::RejectFilesByGlob(ref globs, ref _glob_set) => serializer
				.serialize_newtype_variant("ParametersPerKind", 1, "RejectFilesByGlob", globs),
			ParametersPerKind::AcceptIfChildrenDirectoriesArePresent(ref children) => serializer
				.serialize_newtype_variant(
					"ParametersPerKind",
					2,
					"AcceptIfChildrenDirectoriesArePresent",
					children,
				),
			ParametersPerKind::RejectIfChildrenDirectoriesArePresent(ref children) => serializer
				.serialize_newtype_variant(
					"ParametersPerKind",
					3,
					"RejectIfChildrenDirectoriesArePresent",
					children,
				),
		}
	}
}

impl<'de> Deserialize<'de> for ParametersPerKind {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: de::Deserializer<'de>,
	{
		const VARIANTS: &[&str] = &[
			"AcceptFilesByGlob",
			"RejectFilesByGlob",
			"AcceptIfChildrenDirectoriesArePresent",
			"RejectIfChildrenDirectoriesArePresent",
		];

		enum Fields {
			AcceptFilesByGlob,
			RejectFilesByGlob,
			AcceptIfChildrenDirectoriesArePresent,
			RejectIfChildrenDirectoriesArePresent,
		}

		struct FieldsVisitor;

		impl<'de> de::Visitor<'de> for FieldsVisitor {
			type Value = Fields;

			fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
				formatter.write_str(
					"`AcceptFilesByGlob` \
				or `RejectFilesByGlob` \
				or `AcceptIfChildrenDirectoriesArePresent` \
				or `RejectIfChildrenDirectoriesArePresent`",
				)
			}

			fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
			where
				E: de::Error,
			{
				match value {
					0 => Ok(Fields::AcceptFilesByGlob),
					1 => Ok(Fields::RejectFilesByGlob),
					2 => Ok(Fields::AcceptIfChildrenDirectoriesArePresent),
					3 => Ok(Fields::RejectIfChildrenDirectoriesArePresent),
					_ => Err(de::Error::invalid_value(
						de::Unexpected::Unsigned(value),
						&"variant index 0 <= i < 3",
					)),
				}
			}
			fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
			where
				E: de::Error,
			{
				match value {
					"AcceptFilesByGlob" => Ok(Fields::AcceptFilesByGlob),
					"RejectFilesByGlob" => Ok(Fields::RejectFilesByGlob),
					"AcceptIfChildrenDirectoriesArePresent" => {
						Ok(Fields::AcceptIfChildrenDirectoriesArePresent)
					}
					"RejectIfChildrenDirectoriesArePresent" => {
						Ok(Fields::RejectIfChildrenDirectoriesArePresent)
					}
					_ => Err(de::Error::unknown_variant(value, VARIANTS)),
				}
			}
			fn visit_bytes<E>(self, bytes: &[u8]) -> Result<Self::Value, E>
			where
				E: de::Error,
			{
				match bytes {
					b"AcceptFilesByGlob" => Ok(Fields::AcceptFilesByGlob),
					b"RejectFilesByGlob" => Ok(Fields::RejectFilesByGlob),
					b"AcceptIfChildrenDirectoriesArePresent" => {
						Ok(Fields::AcceptIfChildrenDirectoriesArePresent)
					}
					b"RejectIfChildrenDirectoriesArePresent" => {
						Ok(Fields::RejectIfChildrenDirectoriesArePresent)
					}
					_ => Err(de::Error::unknown_variant(
						&String::from_utf8_lossy(bytes),
						VARIANTS,
					)),
				}
			}
		}

		impl<'de> Deserialize<'de> for Fields {
			#[inline]
			fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
			where
				D: de::Deserializer<'de>,
			{
				deserializer.deserialize_identifier(FieldsVisitor)
			}
		}

		struct ParametersPerKindVisitor<'de> {
			marker: PhantomData<ParametersPerKind>,
			lifetime: PhantomData<&'de ()>,
		}

		impl<'de> de::Visitor<'de> for ParametersPerKindVisitor<'de> {
			type Value = ParametersPerKind;

			fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
				formatter.write_str("enum ParametersPerKind")
			}

			fn visit_enum<PPK>(self, data: PPK) -> Result<Self::Value, PPK::Error>
			where
				PPK: de::EnumAccess<'de>,
			{
				use de::Error;

				de::EnumAccess::variant(data).and_then(|value| match value {
					(Fields::AcceptFilesByGlob, accept_files_by_glob) => {
						de::VariantAccess::newtype_variant::<Vec<Glob>>(accept_files_by_glob)
							.and_then(|globs| {
								globs
									.iter()
									.fold(&mut GlobSetBuilder::new(), |builder, glob| {
										builder.add(glob.to_owned())
									})
									.build()
									.map_or_else(
										|e| Err(PPK::Error::custom(e)),
										|glob_set| {
											Ok(Self::Value::AcceptFilesByGlob(globs, glob_set))
										},
									)
							})
					}
					(Fields::RejectFilesByGlob, reject_files_by_glob) => {
						de::VariantAccess::newtype_variant::<Vec<Glob>>(reject_files_by_glob)
							.and_then(|globs| {
								globs
									.iter()
									.fold(&mut GlobSetBuilder::new(), |builder, glob| {
										builder.add(glob.to_owned())
									})
									.build()
									.map_or_else(
										|e| Err(PPK::Error::custom(e)),
										|glob_set| {
											Ok(Self::Value::RejectFilesByGlob(globs, glob_set))
										},
									)
							})
					}
					(
						Fields::AcceptIfChildrenDirectoriesArePresent,
						accept_if_children_directories_are_present,
					) => de::VariantAccess::newtype_variant::<HashSet<String>>(
						accept_if_children_directories_are_present,
					)
					.map(Self::Value::AcceptIfChildrenDirectoriesArePresent),
					(
						Fields::RejectIfChildrenDirectoriesArePresent,
						reject_if_children_directories_are_present,
					) => de::VariantAccess::newtype_variant::<HashSet<String>>(
						reject_if_children_directories_are_present,
					)
					.map(Self::Value::RejectIfChildrenDirectoriesArePresent),
				})
			}
		}

		deserializer.deserialize_enum(
			"ParametersPerKind",
			VARIANTS,
			ParametersPerKindVisitor {
				marker: PhantomData::<ParametersPerKind>,
				lifetime: PhantomData,
			},
		)
	}
}

impl ParametersPerKind {
	async fn apply(&self, source: impl AsRef<Path>) -> Result<bool, IndexerRuleError> {
		match self {
			ParametersPerKind::AcceptIfChildrenDirectoriesArePresent(children) => {
				accept_dir_for_its_children(source, children).await
			}
			ParametersPerKind::RejectIfChildrenDirectoriesArePresent(children) => {
				reject_dir_for_its_children(source, children).await
			}

			ParametersPerKind::AcceptFilesByGlob(_globs, accept_glob_set) => {
				Ok(accept_by_glob(source, accept_glob_set))
			}
			ParametersPerKind::RejectFilesByGlob(_globs, reject_glob_set) => {
				Ok(reject_by_glob(source, reject_glob_set))
			}
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexerRule {
	pub id: Option<i32>,
	pub kind: RuleKind,
	pub name: String,
	pub default: bool,
	pub parameters: ParametersPerKind,
	pub date_created: DateTime<Utc>,
	pub date_modified: DateTime<Utc>,
}

impl IndexerRule {
	pub fn new(kind: RuleKind, name: String, default: bool, parameters: ParametersPerKind) -> Self {
		Self {
			id: None,
			kind,
			name,
			default,
			parameters,
			date_created: Utc::now(),
			date_modified: Utc::now(),
		}
	}

	pub async fn apply(&self, source: impl AsRef<Path>) -> Result<bool, IndexerRuleError> {
		self.parameters.apply(source).await
	}

	pub async fn save(self, client: &PrismaClient) -> Result<(), IndexerRuleError> {
		if let Some(id) = self.id {
			client
				.indexer_rule()
				.upsert(
					indexer_rule::id::equals(id),
					indexer_rule::create(
						self.kind as i32,
						self.name,
						rmp_serde::to_vec_named(&self.parameters)?,
						vec![indexer_rule::default::set(self.default)],
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
					rmp_serde::to_vec_named(&self.parameters)?,
					vec![indexer_rule::default::set(self.default)],
				)
				.exec()
				.await?;
		}

		Ok(())
	}
}

impl TryFrom<&indexer_rule::Data> for IndexerRule {
	type Error = IndexerRuleError;

	fn try_from(data: &indexer_rule::Data) -> Result<Self, Self::Error> {
		let kind = RuleKind::try_from(data.kind)?;

		Ok(Self {
			id: Some(data.id),
			kind,
			name: data.name.clone(),
			default: data.default,
			parameters: rmp_serde::from_slice(&data.parameters)?,
			date_created: data.date_created.into(),
			date_modified: data.date_modified.into(),
		})
	}
}

impl TryFrom<indexer_rule::Data> for IndexerRule {
	type Error = IndexerRuleError;

	fn try_from(data: indexer_rule::Data) -> Result<Self, Self::Error> {
		Self::try_from(&data)
	}
}

fn accept_by_glob(source: impl AsRef<Path>, accept_glob_set: &GlobSet) -> bool {
	accept_glob_set.is_match(source.as_ref())
}

fn reject_by_glob(source: impl AsRef<Path>, reject_glob_set: &GlobSet) -> bool {
	!accept_by_glob(source.as_ref(), reject_glob_set)
}

async fn accept_dir_for_its_children(
	source: impl AsRef<Path>,
	children: &HashSet<String>,
) -> Result<bool, IndexerRuleError> {
	let source = source.as_ref();
	let mut read_dir = fs::read_dir(source)
		.await
		.map_err(|e| IndexerRuleError::AcceptByItsChildrenFileIO(FileIOError::from((source, e))))?;
	while let Some(entry) = read_dir
		.next_entry()
		.await
		.map_err(|e| IndexerRuleError::AcceptByItsChildrenFileIO(FileIOError::from((source, e))))?
	{
		if entry
			.metadata()
			.await
			.map_err(|e| {
				IndexerRuleError::AcceptByItsChildrenFileIO(FileIOError::from((source, e)))
			})?
			.is_dir() && children.contains(
			entry
				.file_name()
				.to_str()
				.ok_or_else(|| NonUtf8PathError(entry.path().into()))?,
		) {
			return Ok(true);
		}
	}

	Ok(false)
}

async fn reject_dir_for_its_children(
	source: impl AsRef<Path>,
	children: &HashSet<String>,
) -> Result<bool, IndexerRuleError> {
	let source = source.as_ref();
	let mut read_dir = fs::read_dir(source)
		.await
		.map_err(|e| IndexerRuleError::RejectByItsChildrenFileIO(FileIOError::from((source, e))))?;
	while let Some(entry) = read_dir
		.next_entry()
		.await
		.map_err(|e| IndexerRuleError::RejectByItsChildrenFileIO(FileIOError::from((source, e))))?
	{
		if entry
			.metadata()
			.await
			.map_err(|e| {
				IndexerRuleError::RejectByItsChildrenFileIO(FileIOError::from((source, e)))
			})?
			.is_dir() && children.contains(
			entry
				.file_name()
				.to_str()
				.ok_or_else(|| NonUtf8PathError(entry.path().into()))?,
		) {
			return Ok(false);
		}
	}

	Ok(true)
}

pub fn aggregate_rules_by_kind<'r>(
	mut rules: impl Iterator<Item = &'r location_with_indexer_rules::indexer_rules::Data>,
) -> Result<HashMap<RuleKind, Vec<IndexerRule>>, IndexerRuleError> {
	rules.try_fold(
		HashMap::<_, Vec<_>>::with_capacity(RuleKind::variant_count()),
		|mut rules_by_kind, location_rule| {
			IndexerRule::try_from(&location_rule.indexer_rule).map(|rule| {
				rules_by_kind.entry(rule.kind).or_default().push(rule);
				rules_by_kind
			})
		},
	)
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
			false,
			ParametersPerKind::RejectFilesByGlob(
				vec![],
				GlobSetBuilder::new()
					.add(Glob::new("**/.*").unwrap())
					.build()
					.unwrap(),
			),
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
			false,
			ParametersPerKind::RejectFilesByGlob(
				vec![],
				GlobSetBuilder::new()
					.add(Glob::new("{**/target/*,**/target}").unwrap())
					.build()
					.unwrap(),
			),
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
			false,
			ParametersPerKind::AcceptFilesByGlob(
				vec![],
				GlobSetBuilder::new()
					.add(Glob::new("*.{jpg,png,jpeg}").unwrap())
					.build()
					.unwrap(),
			),
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
			false,
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
			false,
			ParametersPerKind::RejectIfChildrenDirectoriesArePresent(childrens),
		);

		assert!(!rule.apply(project1).await.unwrap());
		assert!(!rule.apply(project2).await.unwrap());
		assert!(rule.apply(not_project).await.unwrap());
	}

	impl PartialEq for ParametersPerKind {
		fn eq(&self, other: &Self) -> bool {
			match (self, other) {
				(
					ParametersPerKind::AcceptFilesByGlob(self_globs, _),
					ParametersPerKind::AcceptFilesByGlob(other_globs, _),
				) => self_globs == other_globs,
				(
					ParametersPerKind::RejectFilesByGlob(self_globs, _),
					ParametersPerKind::RejectFilesByGlob(other_globs, _),
				) => self_globs == other_globs,
				(
					ParametersPerKind::AcceptIfChildrenDirectoriesArePresent(self_childrens),
					ParametersPerKind::AcceptIfChildrenDirectoriesArePresent(other_childrens),
				) => self_childrens == other_childrens,
				(
					ParametersPerKind::RejectIfChildrenDirectoriesArePresent(self_childrens),
					ParametersPerKind::RejectIfChildrenDirectoriesArePresent(other_childrens),
				) => self_childrens == other_childrens,
				_ => false,
			}
		}
	}

	impl Eq for ParametersPerKind {}

	impl PartialEq for IndexerRule {
		fn eq(&self, other: &Self) -> bool {
			self.id == other.id
				&& self.kind == other.kind
				&& self.name == other.name
				&& self.default == other.default
				&& self.parameters == other.parameters
				&& self.date_created == other.date_created
				&& self.date_modified == other.date_modified
		}
	}

	impl Eq for IndexerRule {}

	#[test]
	fn serde_smoke_test() {
		let actual = IndexerRule::new(
			RuleKind::RejectFilesByGlob,
			"No Hidden".to_string(),
			true,
			ParametersPerKind::RejectFilesByGlob(
				vec![Glob::new("**/.*").unwrap()],
				Glob::new("**/.*")
					.and_then(|glob| GlobSetBuilder::new().add(glob).build())
					.unwrap(),
			),
		);

		let expected =
			rmp_serde::from_slice::<IndexerRule>(&rmp_serde::to_vec_named(&actual).unwrap())
				.unwrap();

		assert_eq!(actual, expected);
	}
}
