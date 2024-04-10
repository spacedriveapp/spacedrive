#![warn(
	clippy::all,
	clippy::pedantic,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::complexity,
	clippy::nursery,
	clippy::unwrap_used,
	unused_qualifications,
	rust_2018_idioms,
	trivial_casts,
	trivial_numeric_casts,
	unused_allocation,
	clippy::unnecessary_cast,
	clippy::cast_lossless,
	clippy::cast_possible_truncation,
	clippy::cast_possible_wrap,
	clippy::cast_precision_loss,
	clippy::cast_sign_loss,
	clippy::dbg_macro,
	clippy::deprecated_cfg_attr,
	clippy::separated_literal_suffix,
	deprecated
)]
#![forbid(deprecated_in_future)]
#![allow(clippy::missing_errors_doc)]

use sd_prisma::prisma::{indexer_rule, PrismaClient};
use sd_utils::{
	db::{maybe_missing, MissingFieldError},
	error::{FileIOError, NonUtf8PathError},
};
use serde::{Deserialize, Serialize};

use std::{
	collections::{HashMap, HashSet},
	fs::Metadata,
	path::Path,
	sync::Arc,
};

use chrono::{DateTime, Utc};
use futures_concurrency::future::TryJoin;
use globset::{Glob, GlobSet, GlobSetBuilder};
use rmp_serde::{decode, encode};
use rspc::ErrorCode;

use specta::Type;
use thiserror::Error;
use tokio::{fs, sync::RwLock};
use tracing::debug;
use uuid::Uuid;

pub mod seed;
mod serde_impl;

#[derive(Error, Debug)]
pub enum IndexerRuleError {
	// User errors
	#[error("invalid indexer rule kind integer: {0}")]
	InvalidRuleKindInt(i32),
	#[error("glob builder error: {0}")]
	Glob(#[from] globset::Error),
	#[error(transparent)]
	NonUtf8Path(#[from] NonUtf8PathError),

	// Internal Errors
	#[error("indexer rule parameters encode error: {0}")]
	RuleParametersRMPEncode(#[from] encode::Error),
	#[error("indexer rule parameters decode error: {0}")]
	RuleParametersRMPDecode(#[from] decode::Error),
	#[error("accept by its children file I/O error: {0}")]
	AcceptByItsChildrenFileIO(FileIOError),
	#[error("reject by its children file I/O error: {0}")]
	RejectByItsChildrenFileIO(FileIOError),
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("missing-field: {0}")]
	MissingField(#[from] MissingFieldError),
}

impl From<IndexerRuleError> for rspc::Error {
	fn from(err: IndexerRuleError) -> Self {
		match err {
			IndexerRuleError::InvalidRuleKindInt(_)
			| IndexerRuleError::Glob(_)
			| IndexerRuleError::NonUtf8Path(_) => {
				Self::with_cause(ErrorCode::BadRequest, err.to_string(), err)
			}

			_ => Self::with_cause(ErrorCode::InternalServerError, err.to_string(), err),
		}
	}
}

/// `IndexerRuleCreateArgs` is the argument received from the client using rspc to create a new indexer rule.
/// Note that `rules` field is a vector of tuples of `RuleKind` and `parameters`.
///
/// In case of  `RuleKind::AcceptFilesByGlob` or `RuleKind::RejectFilesByGlob`, it will be a
/// vector of strings containing a glob patterns.
///
/// In case of `RuleKind::AcceptIfChildrenDirectoriesArePresent` or `RuleKind::RejectIfChildrenDirectoriesArePresent` the
/// `parameters` field must be a vector of strings containing the names of the directories.
#[derive(Type, Deserialize)]
pub struct IndexerRuleCreateArgs {
	pub name: String,
	pub dry_run: bool,
	pub rules: Vec<(RuleKind, Vec<String>)>,
}

impl IndexerRuleCreateArgs {
	pub async fn create(
		self,
		db: &PrismaClient,
	) -> Result<Option<indexer_rule::Data>, IndexerRuleError> {
		use indexer_rule::{date_created, date_modified, name, rules_per_kind};

		debug!(
			"{} a new indexer rule (name = {}, params = {:?})",
			if self.dry_run {
				"Dry run: Would create"
			} else {
				"Trying to create"
			},
			self.name,
			self.rules
		);

		let rules_data = rmp_serde::to_vec_named(
			&self
				.rules
				.into_iter()
				.map(|(kind, parameters)| match kind {
					RuleKind::AcceptFilesByGlob => {
						RulePerKind::new_accept_files_by_globs_str(parameters)
					}
					RuleKind::RejectFilesByGlob => {
						RulePerKind::new_reject_files_by_globs_str(parameters)
					}
					RuleKind::AcceptIfChildrenDirectoriesArePresent => {
						Ok(RulePerKind::AcceptIfChildrenDirectoriesArePresent(
							parameters.into_iter().collect(),
						))
					}
					RuleKind::RejectIfChildrenDirectoriesArePresent => {
						Ok(RulePerKind::RejectIfChildrenDirectoriesArePresent(
							parameters.into_iter().collect(),
						))
					}
				})
				.collect::<Result<Vec<_>, _>>()?,
		)?;

		if self.dry_run {
			return Ok(None);
		}

		let date_created = Utc::now();

		Ok(Some(
			db.indexer_rule()
				.create(
					sd_utils::uuid_to_bytes(generate_pub_id()),
					vec![
						name::set(Some(self.name)),
						rules_per_kind::set(Some(rules_data)),
						date_created::set(Some(date_created.into())),
						date_modified::set(Some(date_created.into())),
					],
				)
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
	#[must_use]
	pub const fn variant_count() -> usize {
		// TODO: Use https://doc.rust-lang.org/std/mem/fn.variant_count.html if it ever gets stabilized
		4
	}
}

/// `ParametersPerKind` is a mapping from `RuleKind` to the parameters required for each kind of rule.
/// In case of doubt about globs, consult <https://docs.rs/globset/latest/globset/#syntax>
///
/// We store directly globs in the database, serialized using [rmp_serde](https://docs.rs/rmp-serde).
///
/// In case of `ParametersPerKind::AcceptIfChildrenDirectoriesArePresent` or
/// `ParametersPerKind::RejectIfChildrenDirectoriesArePresent`
/// first we change the data structure to a vector, then we serialize it.
#[derive(Debug)]
pub enum RulePerKind {
	// TODO: Add an indexer rule that filter files based on their extended attributes
	// https://learn.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants
	// https://en.wikipedia.org/wiki/Extended_file_attributes
	AcceptFilesByGlob(Vec<Glob>, GlobSet),
	RejectFilesByGlob(Vec<Glob>, GlobSet),
	AcceptIfChildrenDirectoriesArePresent(HashSet<String>),
	RejectIfChildrenDirectoriesArePresent(HashSet<String>),
}

impl RulePerKind {
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

	pub fn new_reject_files_by_globs_str(
		globs_str: impl IntoIterator<Item = impl AsRef<str>>,
	) -> Result<Self, IndexerRuleError> {
		Self::new_files_by_globs_str_and_kind(globs_str, Self::RejectFilesByGlob)
	}
}

pub trait MetadataForIndexerRules: Send + Sync + 'static {
	fn is_dir(&self) -> bool;
}

impl MetadataForIndexerRules for Metadata {
	fn is_dir(&self) -> bool {
		self.is_dir()
	}
}

impl RulePerKind {
	#[deprecated = "Use `[apply_with_metadata]` instead"]
	async fn apply(
		&self,
		source: impl AsRef<Path> + Send,
	) -> Result<(RuleKind, bool), IndexerRuleError> {
		match self {
			Self::AcceptIfChildrenDirectoriesArePresent(children) => {
				accept_dir_for_its_children(source, children)
					.await
					.map(|accepted| (RuleKind::AcceptIfChildrenDirectoriesArePresent, accepted))
			}
			Self::RejectIfChildrenDirectoriesArePresent(children) => {
				reject_dir_for_its_children(source, children)
					.await
					.map(|rejected| (RuleKind::RejectIfChildrenDirectoriesArePresent, rejected))
			}

			Self::AcceptFilesByGlob(_globs, accept_glob_set) => Ok((
				RuleKind::AcceptFilesByGlob,
				accept_by_glob(source, accept_glob_set),
			)),
			Self::RejectFilesByGlob(_globs, reject_glob_set) => Ok((
				RuleKind::RejectFilesByGlob,
				reject_by_glob(source, reject_glob_set),
			)),
		}
	}

	async fn apply_with_metadata(
		&self,
		source: impl AsRef<Path> + Send,
		metadata: &impl MetadataForIndexerRules,
	) -> Result<(RuleKind, bool), IndexerRuleError> {
		match self {
			Self::AcceptIfChildrenDirectoriesArePresent(children) => {
				accept_dir_for_its_children_with_metadata(source, metadata, children)
					.await
					.map(|accepted| (RuleKind::AcceptIfChildrenDirectoriesArePresent, accepted))
			}
			Self::RejectIfChildrenDirectoriesArePresent(children) => {
				reject_dir_for_its_children_with_metadata(source, metadata, children)
					.await
					.map(|rejected| (RuleKind::RejectIfChildrenDirectoriesArePresent, rejected))
			}

			Self::AcceptFilesByGlob(_globs, accept_glob_set) => Ok((
				RuleKind::AcceptFilesByGlob,
				accept_by_glob(source, accept_glob_set),
			)),
			Self::RejectFilesByGlob(_globs, reject_glob_set) => Ok((
				RuleKind::RejectFilesByGlob,
				reject_by_glob(source, reject_glob_set),
			)),
		}
	}
}

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
	#[deprecated = "Use `[apply_with_metadata]` instead"]
	pub async fn apply(
		&self,
		source: impl AsRef<Path> + Send,
	) -> Result<Vec<(RuleKind, bool)>, IndexerRuleError> {
		self.rules
			.iter()
			.map(|rule| rule.apply(source.as_ref()))
			.collect::<Vec<_>>()
			.try_join()
			.await
	}

	pub async fn apply_with_metadata(
		&self,
		source: impl AsRef<Path> + Send,
		metadata: &impl MetadataForIndexerRules,
	) -> Result<Vec<(RuleKind, bool)>, IndexerRuleError> {
		async fn inner(
			rules: &[RulePerKind],
			source: &Path,
			metadata: &impl MetadataForIndexerRules,
		) -> Result<Vec<(RuleKind, bool)>, IndexerRuleError> {
			rules
				.iter()
				.map(|rule| rule.apply_with_metadata(source, metadata))
				.collect::<Vec<_>>()
				.try_join()
				.await
		}

		inner(&self.rules, source.as_ref(), metadata).await
	}

	#[deprecated = "Use `[IndexerRuler::apply_all]` instead"]
	pub async fn apply_all(
		rules: &[Self],
		source: impl AsRef<Path> + Send,
	) -> Result<HashMap<RuleKind, Vec<bool>>, IndexerRuleError> {
		rules
			.iter()
			.map(|rule| rule.apply(source.as_ref()))
			.collect::<Vec<_>>()
			.try_join()
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

#[derive(Debug, Clone, Default)]
pub struct IndexerRuler {
	// TODO(fogodev): Use this RwLock later to acquire new rules while applying rules, like from a .gitignore file
	rules: Arc<RwLock<Vec<IndexerRule>>>,
}

impl IndexerRuler {
	#[must_use]
	pub fn new(rules: Vec<IndexerRule>) -> Self {
		Self {
			rules: Arc::new(RwLock::new(rules)),
		}
	}

	pub async fn serialize(&self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
		rmp_serde::to_vec_named(&*self.rules.read().await)
	}

	pub fn deserialize(data: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
		rmp_serde::from_slice(data).map(Self::new)
	}

	pub async fn apply_all(
		&self,
		source: impl AsRef<Path> + Send,
		metadata: &impl MetadataForIndexerRules,
	) -> Result<HashMap<RuleKind, Vec<bool>>, IndexerRuleError> {
		async fn inner(
			rules: &[IndexerRule],
			source: &Path,
			metadata: &impl MetadataForIndexerRules,
		) -> Result<HashMap<RuleKind, Vec<bool>>, IndexerRuleError> {
			rules
				.iter()
				.map(|rule| rule.apply_with_metadata(source, metadata))
				.collect::<Vec<_>>()
				.try_join()
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

		inner(&self.rules.read().await, source.as_ref(), metadata).await
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

fn accept_by_glob(source: impl AsRef<Path>, accept_glob_set: &GlobSet) -> bool {
	accept_glob_set.is_match(source.as_ref())
}

fn reject_by_glob(source: impl AsRef<Path>, reject_glob_set: &GlobSet) -> bool {
	!accept_by_glob(source.as_ref(), reject_glob_set)
}

#[deprecated = "Use `[accept_dir_for_its_children_with_metadata]` instead"]
async fn accept_dir_for_its_children(
	source: impl AsRef<Path> + Send,
	children: &HashSet<String>,
) -> Result<bool, IndexerRuleError> {
	let source = source.as_ref();

	// FIXME(fogodev): Just check for io::ErrorKind::NotADirectory error instead (feature = "io_error_more", issue = "86442")
	if !fs::metadata(source)
		.await
		.map_err(|e| IndexerRuleError::AcceptByItsChildrenFileIO(FileIOError::from((source, e))))?
		.is_dir()
	{
		return Ok(false);
	}

	let mut read_dir = fs::read_dir(source)
		.await // TODO: Check NotADirectory error here when available
		.map_err(|e| IndexerRuleError::AcceptByItsChildrenFileIO(FileIOError::from((source, e))))?;
	while let Some(entry) = read_dir
		.next_entry()
		.await
		.map_err(|e| IndexerRuleError::AcceptByItsChildrenFileIO(FileIOError::from((source, e))))?
	{
		let entry_name = entry
			.file_name()
			.to_str()
			.ok_or_else(|| NonUtf8PathError(entry.path().into()))?
			.to_string();

		if entry
			.metadata()
			.await
			.map_err(|e| {
				IndexerRuleError::AcceptByItsChildrenFileIO(FileIOError::from((source, e)))
			})?
			.is_dir() && children.contains(&entry_name)
		{
			return Ok(true);
		}
	}

	Ok(false)
}

async fn accept_dir_for_its_children_with_metadata(
	source: impl AsRef<Path> + Send,
	metadata: &impl MetadataForIndexerRules,
	children: &HashSet<String>,
) -> Result<bool, IndexerRuleError> {
	let source = source.as_ref();

	// FIXME(fogodev): Just check for io::ErrorKind::NotADirectory error instead (feature = "io_error_more", issue = "86442")
	if !metadata.is_dir() {
		return Ok(false);
	}

	let mut read_dir = fs::read_dir(source)
		.await // TODO: Check NotADirectory error here when available
		.map_err(|e| IndexerRuleError::AcceptByItsChildrenFileIO(FileIOError::from((source, e))))?;
	while let Some(entry) = read_dir
		.next_entry()
		.await
		.map_err(|e| IndexerRuleError::AcceptByItsChildrenFileIO(FileIOError::from((source, e))))?
	{
		let entry_name = entry
			.file_name()
			.to_str()
			.ok_or_else(|| NonUtf8PathError(entry.path().into()))?
			.to_string();

		if entry
			.metadata()
			.await
			.map_err(|e| {
				IndexerRuleError::AcceptByItsChildrenFileIO(FileIOError::from((source, e)))
			})?
			.is_dir() && children.contains(&entry_name)
		{
			return Ok(true);
		}
	}

	Ok(false)
}

#[deprecated = "Use `[reject_dir_for_its_children_with_metadata]` instead"]
async fn reject_dir_for_its_children(
	source: impl AsRef<Path> + Send,
	children: &HashSet<String>,
) -> Result<bool, IndexerRuleError> {
	let source = source.as_ref();

	// FIXME(fogodev): Just check for io::ErrorKind::NotADirectory error instead (feature = "io_error_more", issue = "86442")
	if !fs::metadata(source)
		.await
		.map_err(|e| IndexerRuleError::AcceptByItsChildrenFileIO(FileIOError::from((source, e))))?
		.is_dir()
	{
		return Ok(true);
	}

	let mut read_dir = fs::read_dir(source)
		.await // TODO: Check NotADirectory error here when available
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

async fn reject_dir_for_its_children_with_metadata(
	source: impl AsRef<Path> + Send,
	metadata: &impl MetadataForIndexerRules,
	children: &HashSet<String>,
) -> Result<bool, IndexerRuleError> {
	let source = source.as_ref();

	// FIXME(fogodev): Just check for io::ErrorKind::NotADirectory error instead (feature = "io_error_more", issue = "86442")
	if !metadata.is_dir() {
		return Ok(true);
	}

	let mut read_dir = fs::read_dir(source)
		.await // TODO: Check NotADirectory error here when available
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

#[must_use]
pub fn generate_pub_id() -> Uuid {
	loop {
		let pub_id = Uuid::new_v4();
		if pub_id.as_u128() >= 0xFFF {
			return pub_id;
		}
	}
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
	use super::*;
	use tempfile::tempdir;

	impl IndexerRule {
		#[must_use]
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

	async fn check_rule(indexer_rule: &IndexerRule, path: impl AsRef<Path> + Send) -> bool {
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
	#[allow(clippy::similar_names)]
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

		let childrens = HashSet::from([".git".to_string()]);

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

		let childrens = HashSet::from([".git".to_string()]);

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
					Self::AcceptFilesByGlob(self_globs, _),
					Self::AcceptFilesByGlob(other_globs, _),
				)
				| (
					Self::RejectFilesByGlob(self_globs, _),
					Self::RejectFilesByGlob(other_globs, _),
				) => self_globs == other_globs,

				(
					Self::AcceptIfChildrenDirectoriesArePresent(self_childrens),
					Self::AcceptIfChildrenDirectoriesArePresent(other_childrens),
				)
				| (
					Self::RejectIfChildrenDirectoriesArePresent(self_childrens),
					Self::RejectIfChildrenDirectoriesArePresent(other_childrens),
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
