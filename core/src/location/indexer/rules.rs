use crate::{
	library::Library,
	prisma::indexer_rule,
	util::{
		db::{maybe_missing, uuid_to_bytes, MissingFieldError},
		error::{FileIOError, NonUtf8PathError},
	},
};

use std::{
	collections::{HashMap, HashSet},
	marker::PhantomData,
	path::Path,
};

use chrono::{DateTime, Utc};
use futures::future::try_join_all;
use globset::{Glob, GlobSet, GlobSetBuilder};
use rmp_serde::{self, decode, encode};
use rspc::ErrorCode;
use serde::{de, ser, Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use tokio::fs;
use tracing::debug;
use uuid::Uuid;

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
				rspc::Error::with_cause(ErrorCode::BadRequest, err.to_string(), err)
			}

			_ => rspc::Error::with_cause(ErrorCode::InternalServerError, err.to_string(), err),
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

		use indexer_rule::*;

		Ok(Some(
			library
				.db
				.indexer_rule()
				.create(
					uuid_to_bytes(generate_pub_id()),
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
	pub const fn variant_count() -> usize {
		// TODO: Use https://doc.rust-lang.org/std/mem/fn.variant_count.html if it ever gets stabilized
		4
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

/// We're implementing `Serialize` by hand as `GlobSet`s aren't serializable, so we ignore them on
/// serialization
impl Serialize for RulePerKind {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: ser::Serializer,
	{
		match *self {
			RulePerKind::AcceptFilesByGlob(ref globs, ref _glob_set) => serializer
				.serialize_newtype_variant("ParametersPerKind", 0, "AcceptFilesByGlob", globs),
			RulePerKind::RejectFilesByGlob(ref globs, ref _glob_set) => serializer
				.serialize_newtype_variant("ParametersPerKind", 1, "RejectFilesByGlob", globs),
			RulePerKind::AcceptIfChildrenDirectoriesArePresent(ref children) => serializer
				.serialize_newtype_variant(
					"ParametersPerKind",
					2,
					"AcceptIfChildrenDirectoriesArePresent",
					children,
				),
			RulePerKind::RejectIfChildrenDirectoriesArePresent(ref children) => serializer
				.serialize_newtype_variant(
					"ParametersPerKind",
					3,
					"RejectIfChildrenDirectoriesArePresent",
					children,
				),
		}
	}
}

impl<'de> Deserialize<'de> for RulePerKind {
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
			marker: PhantomData<RulePerKind>,
			lifetime: PhantomData<&'de ()>,
		}

		impl<'de> de::Visitor<'de> for ParametersPerKindVisitor<'de> {
			type Value = RulePerKind;

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
				marker: PhantomData::<RulePerKind>,
				lifetime: PhantomData,
			},
		)
	}
}

impl RulePerKind {
	async fn apply(&self, source: impl AsRef<Path>) -> Result<(RuleKind, bool), IndexerRuleError> {
		match self {
			RulePerKind::AcceptIfChildrenDirectoriesArePresent(children) => {
				accept_dir_for_its_children(source, children)
					.await
					.map(|accepted| (RuleKind::AcceptIfChildrenDirectoriesArePresent, accepted))
			}
			RulePerKind::RejectIfChildrenDirectoriesArePresent(children) => {
				reject_dir_for_its_children(source, children)
					.await
					.map(|rejected| (RuleKind::RejectIfChildrenDirectoriesArePresent, rejected))
			}

			RulePerKind::AcceptFilesByGlob(_globs, accept_glob_set) => Ok((
				RuleKind::AcceptFilesByGlob,
				accept_by_glob(source, accept_glob_set),
			)),
			RulePerKind::RejectFilesByGlob(_globs, reject_glob_set) => Ok((
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
					HashMap::with_capacity(RuleKind::variant_count()),
					|mut map, (kind, result)| {
						map.entry(kind).or_insert_with(Vec::new).push(result);
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
			default: maybe_missing(data.default, "indexer_rule.default")?,
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

async fn accept_dir_for_its_children(
	source: impl AsRef<Path>,
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

async fn reject_dir_for_its_children(
	source: impl AsRef<Path>,
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

pub fn generate_pub_id() -> Uuid {
	loop {
		let pub_id = Uuid::new_v4();
		if pub_id.as_u128() >= 0xFFF {
			return pub_id;
		}
	}
}

mod seeder {
	use crate::{
		location::indexer::rules::{IndexerRuleError, RulePerKind},
		prisma::PrismaClient,
		util::db::uuid_to_bytes,
	};
	use chrono::Utc;
	use sd_prisma::prisma::indexer_rule;
	use thiserror::Error;
	use uuid::Uuid;

	#[derive(Error, Debug)]
	pub enum SeederError {
		#[error("Failed to run indexer rules seeder: {0}")]
		IndexerRules(#[from] IndexerRuleError),
		#[error("An error occurred with the database while applying migrations: {0}")]
		DatabaseError(#[from] prisma_client_rust::QueryError),
	}

	struct SystemIndexerRule {
		name: &'static str,
		rules: Vec<RulePerKind>,
		default: bool,
	}

	pub async fn seeder(client: &PrismaClient) -> Result<(), SeederError> {
		// DO NOT REORDER THIS ARRAY!
		for (i, rule) in [
			no_os_protected(),
			no_hidden(),
			only_git_repos(),
			only_images(),
		]
		.into_iter()
		.enumerate()
		{
			let pub_id = uuid_to_bytes(Uuid::from_u128(i as u128));
			let rules = rmp_serde::to_vec_named(&rule.rules).map_err(IndexerRuleError::from)?;

			use indexer_rule::*;

			let data = vec![
				name::set(Some(rule.name.to_string())),
				rules_per_kind::set(Some(rules.clone())),
				default::set(Some(rule.default)),
				date_created::set(Some(Utc::now().into())),
				date_modified::set(Some(Utc::now().into())),
			];

			client
				.indexer_rule()
				.upsert(
					indexer_rule::pub_id::equals(pub_id.clone()),
					indexer_rule::create(pub_id.clone(), data.clone()),
					data,
				)
				.exec()
				.await?;
		}

		Ok(())
	}

	fn no_os_protected() -> SystemIndexerRule {
		SystemIndexerRule {
        // TODO: On windows, beside the listed files, any file with the FILE_ATTRIBUTE_SYSTEM should be considered a system file
        // https://learn.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants#FILE_ATTRIBUTE_SYSTEM
        name: "No OS protected",
        default: true,
        rules: vec![
            RulePerKind::new_reject_files_by_globs_str(
                [
                    vec![
                        "**/.spacedrive",
                    ],
                    // Globset, even on Windows, requires the use of / as a separator
                    // https://github.com/github/gitignore/blob/main/Global/Windows.gitignore
                    // https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file
                    #[cfg(target_os = "windows")]
                    vec![
                        // Windows thumbnail cache files
                        "**/{Thumbs.db,Thumbs.db:encryptable,ehthumbs.db,ehthumbs_vista.db}",
                        // Dump file
                        "**/*.stackdump",
                        // Folder config file
                        "**/[Dd]esktop.ini",
                        // Recycle Bin used on file shares
                        "**/$RECYCLE.BIN",
                        // Chkdsk recovery directory
                        "**/FOUND.[0-9][0-9][0-9]",
                        // Reserved names
                        "**/{CON,PRN,AUX,NUL,COM0,COM1,COM2,COM3,COM4,COM5,COM6,COM7,COM8,COM9,LPT0,LPT1,LPT2,LPT3,LPT4,LPT5,LPT6,LPT7,LPT8,LPT9}",
                        "**/{CON,PRN,AUX,NUL,COM0,COM1,COM2,COM3,COM4,COM5,COM6,COM7,COM8,COM9,LPT0,LPT1,LPT2,LPT3,LPT4,LPT5,LPT6,LPT7,LPT8,LPT9}.*",
                        // User special files
                        "C:/Users/*/NTUSER.DAT*",
                        "C:/Users/*/ntuser.dat*",
                        "C:/Users/*/{ntuser.ini,ntuser.dat,NTUSER.DAT}",
                        // User special folders (most of these the user dont even have permission to access)
                        "C:/Users/*/{Cookies,AppData,NetHood,Recent,PrintHood,SendTo,Templates,Start Menu,Application Data,Local Settings}",
                        // System special folders
                        "C:/{$Recycle.Bin,$WinREAgent,Documents and Settings,Program Files,Program Files (x86),ProgramData,Recovery,PerfLogs,Windows,Windows.old}",
                        // NTFS internal dir, can exists on any drive
                        "[A-Z]:/System Volume Information",
                        // System special files
                        "C:/{config,pagefile,hiberfil}.sys",
                        // Windows can create a swapfile on any drive
                        "[A-Z]:/swapfile.sys",
                        "C:/DumpStack.log.tmp",
                    ],
                    // https://github.com/github/gitignore/blob/main/Global/macOS.gitignore
                    // https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/FileSystemOverview/FileSystemOverview.html#//apple_ref/doc/uid/TP40010672-CH2-SW14
                    #[cfg(any(target_os = "ios", target_os = "macos"))]
                    vec![
                        "**/.{DS_Store,AppleDouble,LSOverride}",
                        // Icon must end with two \r
                        "**/Icon\r\r",
                        // Thumbnails
                        "**/._*",
                    ],
                    #[cfg(target_os = "macos")]
                    vec![
                        "/{System,Network,Library,Applications}",
                        "/Users/*/{Library,Applications}",
						"**/*.photoslibrary/{database,external,private,resources,scope}",
                        // Files that might appear in the root of a volume
                        "**/.{DocumentRevisions-V100,fseventsd,Spotlight-V100,TemporaryItems,Trashes,VolumeIcon.icns,com.apple.timemachine.donotpresent}",
                        // Directories potentially created on remote AFP share
                        "**/.{AppleDB,AppleDesktop,apdisk}",
                        "**/{Network Trash Folder,Temporary Items}",
                    ],
                    // https://github.com/github/gitignore/blob/main/Global/Linux.gitignore
                    #[cfg(target_os = "linux")]
                    vec![
                        "**/*~",
                        // temporary files which can be created if a process still has a handle open of a deleted file
                        "**/.fuse_hidden*",
                        // KDE directory preferences
                        "**/.directory",
                        // Linux trash folder which might appear on any partition or disk
                        "**/.Trash-*",
                        // .nfs files are created when an open file is removed but is still being accessed
                        "**/.nfs*",
                    ],
                    #[cfg(target_os = "android")]
                    vec![
                        "**/.nomedia",
                        "**/.thumbnails",
                    ],
                    // https://en.wikipedia.org/wiki/Unix_filesystem#Conventional_directory_layout
                    // https://en.wikipedia.org/wiki/Filesystem_Hierarchy_Standard
                    #[cfg(target_family = "unix")]
                    vec![
                        // Directories containing unix memory/device mapped files/dirs
                        "/{dev,sys,proc}",
                        // Directories containing special files for current running programs
                        "/{run,var,boot}",
                        // ext2-4 recovery directory
                        "**/lost+found",
                    ],
                ]
                .into_iter()
                .flatten()
            ).expect("this is hardcoded and should always work"),
        ],
    }
	}

	fn no_hidden() -> SystemIndexerRule {
		SystemIndexerRule {
			name: "No Hidden",
			default: true,
			rules: vec![RulePerKind::new_reject_files_by_globs_str(["**/.*"])
				.expect("this is hardcoded and should always work")],
		}
	}

	fn only_git_repos() -> SystemIndexerRule {
		SystemIndexerRule {
			name: "Only Git Repositories",
			default: false,
			rules: vec![RulePerKind::AcceptIfChildrenDirectoriesArePresent(
				[".git".to_string()].into_iter().collect(),
			)],
		}
	}

	fn only_images() -> SystemIndexerRule {
		SystemIndexerRule {
			name: "Only Images",
			default: false,
			rules: vec![RulePerKind::new_accept_files_by_globs_str([
				"*.{avif,bmp,gif,ico,jpeg,jpg,png,svg,tif,tiff,webp}",
			])
			.expect("this is hardcoded and should always work")],
		}
	}
}

pub use seeder::*;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
	use super::*;
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
