use std::{collections::HashSet, marker::PhantomData, path::Path};

use globset::{Glob, GlobSet, GlobSetBuilder};
use sd_utils::error::{FileIOError, NonUtf8PathError};
use serde::{de, ser, Deserialize, Serialize};
use tokio::fs;

use super::{IndexerRuleError, RuleKind};

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
	pub async fn apply(
		&self,
		source: impl AsRef<Path>,
	) -> Result<(RuleKind, bool), IndexerRuleError> {
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

fn accept_by_glob(source: impl AsRef<Path>, accept_glob_set: &GlobSet) -> bool {
	accept_glob_set.is_match(source.as_ref())
}

fn reject_by_glob(source: impl AsRef<Path>, reject_glob_set: &GlobSet) -> bool {
	!accept_by_glob(source.as_ref(), reject_glob_set)
}
