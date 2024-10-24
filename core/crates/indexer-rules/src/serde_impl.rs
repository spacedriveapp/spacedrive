use std::{collections::HashSet, marker::PhantomData};

use globset::{Glob, GlobSetBuilder};
use serde::{de, ser, Deserialize, Serialize};

use super::RulePerKind;

/// We're implementing `Serialize` by hand as `GlobSet`s aren't serializable, so we ignore them on
/// serialization
impl Serialize for RulePerKind {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: ser::Serializer,
	{
		match *self {
			Self::AcceptFilesByGlob(ref globs, ref _glob_set) => serializer
				.serialize_newtype_variant("ParametersPerKind", 0, "AcceptFilesByGlob", globs),
			Self::RejectFilesByGlob(ref globs, ref _glob_set) => serializer
				.serialize_newtype_variant("ParametersPerKind", 1, "RejectFilesByGlob", globs),
			Self::AcceptIfChildrenDirectoriesArePresent(ref children) => serializer
				.serialize_newtype_variant(
					"ParametersPerKind",
					2,
					"AcceptIfChildrenDirectoriesArePresent",
					children,
				),
			Self::RejectIfChildrenDirectoriesArePresent(ref children) => serializer
				.serialize_newtype_variant(
					"ParametersPerKind",
					3,
					"RejectIfChildrenDirectoriesArePresent",
					children,
				),
			Self::IgnoredByGit(_, _) => {
				unreachable!("git ignore rules are dynamic and not serialized")
			}
		}
	}
}

impl<'de> Deserialize<'de> for RulePerKind {
	#[allow(clippy::too_many_lines)]
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

		impl de::Visitor<'_> for FieldsVisitor {
			type Value = Fields;

			fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

			fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
				marker: PhantomData::<Self>,
				lifetime: PhantomData,
			},
		)
	}
}
