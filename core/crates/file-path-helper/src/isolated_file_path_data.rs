use sd_core_prisma_helpers::{
	file_path_for_file_identifier, file_path_for_media_processor, file_path_for_object_validator,
	file_path_to_full_path, file_path_to_handle_custom_uri, file_path_to_handle_p2p_serve_file,
	file_path_to_isolate, file_path_to_isolate_with_id, file_path_to_isolate_with_pub_id,
	file_path_walker, file_path_watcher_remove, file_path_with_object,
};

use sd_prisma::prisma::{file_path, location};
use sd_utils::error::NonUtf8PathError;

use std::{
	borrow::Cow,
	fmt,
	path::{Path, PathBuf, MAIN_SEPARATOR, MAIN_SEPARATOR_STR},
	sync::OnceLock,
};

use regex::RegexSet;
use serde::{Deserialize, Serialize};

use super::FilePathError;

static FORBIDDEN_FILE_NAMES: OnceLock<RegexSet> = OnceLock::new();

#[derive(Debug)]
pub struct IsolatedFilePathDataParts<'a> {
	pub location_id: location::id::Type,
	pub materialized_path: &'a str,
	pub is_dir: bool,
	pub name: &'a str,
	pub extension: &'a str,
	relative_path: &'a str,
}

#[derive(Serialize, Deserialize, Debug, Hash, Eq, PartialEq, Clone, Default)]
#[non_exhaustive]
pub struct IsolatedFilePathData<'a> {
	// WARN! These fields MUST NOT be changed outside the location module, that's why they have this visibility
	// and are not public. They have some specific logic on them and should not be written to directly.
	// If you wanna access one of them outside from location module, write yourself an accessor method
	// to have read only access to them.
	pub(super) location_id: location::id::Type,
	pub(super) materialized_path: Cow<'a, str>,
	pub(super) is_dir: bool,
	pub(super) name: Cow<'a, str>,
	pub(super) extension: Cow<'a, str>,
	relative_path: Cow<'a, str>,
}

impl IsolatedFilePathData<'static> {
	pub fn new(
		location_id: location::id::Type,
		location_path: impl AsRef<Path>,
		full_path: impl AsRef<Path>,
		is_dir: bool,
	) -> Result<Self, FilePathError> {
		let full_path = full_path.as_ref();
		let location_path = location_path.as_ref();

		let extension = (!is_dir)
			.then(|| {
				full_path
					.extension()
					.and_then(|ext| ext.to_str().map(str::to_string))
					.unwrap_or_default()
			})
			.unwrap_or_default();

		Ok(Self {
			is_dir,
			location_id,
			materialized_path: Cow::Owned(extract_normalized_materialized_path_str(
				location_id,
				location_path,
				full_path,
			)?),
			name: Cow::Owned(
				(location_path != full_path)
					.then(|| Self::prepare_name(full_path, is_dir).to_string())
					.unwrap_or_default(),
			),
			extension: Cow::Owned(extension),
			relative_path: Cow::Owned(extract_relative_path(
				location_id,
				location_path,
				full_path,
			)?),
		})
	}
}

impl<'a> IsolatedFilePathData<'a> {
	#[must_use]
	pub const fn location_id(&self) -> location::id::Type {
		self.location_id
	}

	#[must_use]
	pub fn extension(&self) -> &str {
		self.extension.as_ref()
	}

	#[must_use]
	pub fn to_owned(self) -> IsolatedFilePathData<'static> {
		IsolatedFilePathData {
			location_id: self.location_id,
			materialized_path: Cow::Owned(self.materialized_path.to_string()),
			is_dir: self.is_dir,
			name: Cow::Owned(self.name.to_string()),
			extension: Cow::Owned(self.extension.to_string()),
			relative_path: Cow::Owned(self.relative_path.to_string()),
		}
	}

	#[must_use]
	pub const fn is_dir(&self) -> bool {
		self.is_dir
	}

	#[must_use]
	pub fn is_root(&self) -> bool {
		self.is_dir
			&& self.materialized_path == "/"
			&& self.name.is_empty()
			&& self.relative_path.is_empty()
	}

	#[must_use]
	pub fn to_parts(&self) -> IsolatedFilePathDataParts<'_> {
		IsolatedFilePathDataParts {
			location_id: self.location_id,
			materialized_path: &self.materialized_path,
			is_dir: self.is_dir,
			name: &self.name,
			extension: &self.extension,
			relative_path: &self.relative_path,
		}
	}

	/// Return the `IsolatedFilePath` for the parent of the current file or directory.
	///
	/// # Panics
	/// May panic if the materialized path was malformed, without a slash for the parent directory.
	/// Considering that the parent can be just `/` for the root directory.
	#[must_use]
	pub fn parent(&'a self) -> Self {
		let (parent_path_str, name, relative_path) = if self.materialized_path == "/" {
			("/", "", "")
		} else {
			let trailing_slash_idx = self.materialized_path.len() - 1;
			let last_slash_idx = self.materialized_path[..trailing_slash_idx]
				.rfind('/')
				.expect("malformed materialized path at `parent` method");

			(
				&self.materialized_path[..=last_slash_idx],
				&self.materialized_path[last_slash_idx + 1..trailing_slash_idx],
				&self.materialized_path[1..trailing_slash_idx],
			)
		};

		Self {
			is_dir: true,
			location_id: self.location_id,
			relative_path: Cow::Borrowed(relative_path),
			materialized_path: Cow::Borrowed(parent_path_str),
			name: Cow::Borrowed(name),
			extension: Cow::Borrowed(""),
		}
	}

	pub fn from_relative_str(
		location_id: location::id::Type,
		relative_file_path_str: &'a str,
	) -> Self {
		let is_dir = relative_file_path_str.ends_with('/');

		let (materialized_path, maybe_name, maybe_extension) =
			Self::separate_path_name_and_extension_from_str(relative_file_path_str, is_dir);

		Self {
			location_id,
			materialized_path: Cow::Borrowed(materialized_path),
			is_dir,
			name: maybe_name.map(Cow::Borrowed).unwrap_or_default(),
			extension: maybe_extension.map(Cow::Borrowed).unwrap_or_default(),
			relative_path: Cow::Borrowed(relative_file_path_str),
		}
	}

	#[must_use]
	pub fn full_name(&self) -> String {
		if self.extension.is_empty() {
			self.name.to_string()
		} else {
			format!("{}.{}", self.name, self.extension)
		}
	}

	#[must_use]
	pub fn materialized_path_for_children(&self) -> Option<String> {
		if self.materialized_path == "/" && self.name.is_empty() && self.is_dir {
			// We're at the root file_path
			Some("/".to_string())
		} else {
			self.is_dir
				.then(|| format!("{}{}/", self.materialized_path, self.name))
		}
	}

	pub fn separate_name_and_extension_from_str(
		source: &'a str,
	) -> Result<(&'a str, &'a str), FilePathError> {
		if source.contains(MAIN_SEPARATOR) {
			return Err(FilePathError::InvalidFilenameAndExtension(
				source.to_string(),
			));
		}

		source.rfind('.').map_or_else(
			|| Ok((source, "")), // It's a file without extension
			|last_dot_idx| {
				if last_dot_idx == 0 {
					// The dot is the first character, so it's a hidden file
					Ok((source, ""))
				} else {
					Ok((&source[..last_dot_idx], &source[last_dot_idx + 1..]))
				}
			},
		)
	}

	#[allow(clippy::missing_panics_doc)] // Don't actually panic as the regexes are hardcoded
	#[must_use]
	pub fn accept_file_name(name: &str) -> bool {
		let reg = {
			// Maybe we should enforce windows more restrictive rules on all platforms?
			#[cfg(target_os = "windows")]
			{
				FORBIDDEN_FILE_NAMES.get_or_init(|| {
					RegexSet::new([
						r"(?i)^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(\.\w+)*$",
						r#"[<>:"/\\|?*\u0000-\u0031]"#,
					])
					.expect("this regex should always be valid")
				})
			}

			#[cfg(not(target_os = "windows"))]
			{
				FORBIDDEN_FILE_NAMES.get_or_init(|| {
					RegexSet::new([r"/|\x00"]).expect("this regex should always be valid")
				})
			}
		};

		!reg.is_match(name)
	}

	#[must_use]
	pub fn separate_path_name_and_extension_from_str(
		source: &'a str,
		is_dir: bool,
	) -> (
		&'a str,         // Materialized path
		Option<&'a str>, // Maybe a name
		Option<&'a str>, // Maybe an extension
	) {
		let length = source.len();

		if length == 1 {
			// The case for the root path
			(source, None, None)
		} else if is_dir {
			let last_char_idx = if source.ends_with('/') {
				length - 1
			} else {
				length
			};

			let first_name_char_idx = source[..last_char_idx].rfind('/').unwrap_or(0) + 1;
			(
				&source[..first_name_char_idx],
				Some(&source[first_name_char_idx..last_char_idx]),
				None,
			)
		} else {
			let first_name_char_idx = source.rfind('/').unwrap_or(0) + 1;
			let end_idx = first_name_char_idx - 1;
			source[first_name_char_idx..].rfind('.').map_or_else(
				|| {
					(
						&source[..end_idx],
						Some(&source[first_name_char_idx..]),
						None,
					)
				},
				|last_dot_relative_idx| {
					let last_dot_idx = first_name_char_idx + last_dot_relative_idx;
					(
						&source[..end_idx],
						Some(&source[first_name_char_idx..last_dot_idx]),
						Some(&source[last_dot_idx + 1..]),
					)
				},
			)
		}
	}

	fn prepare_name(path: &Path, is_dir: bool) -> &str {
		// Not using `impl AsRef<Path>` here because it's an private method
		if is_dir {
			path.file_name()
		} else {
			path.file_stem()
		}
		.unwrap_or_default()
		.to_str()
		.unwrap_or_default()
	}

	#[must_use]
	pub fn from_db_data(
		location_id: location::id::Type,
		is_dir: bool,
		materialized_path: Cow<'a, str>,
		name: Cow<'a, str>,
		extension: Cow<'a, str>,
	) -> Self {
		Self {
			relative_path: Cow::Owned(assemble_relative_path(
				&materialized_path,
				&name,
				&extension,
				is_dir,
			)),
			location_id,
			materialized_path,
			is_dir,
			name,
			extension,
		}
	}
}

impl AsRef<Path> for IsolatedFilePathData<'_> {
	fn as_ref(&self) -> &Path {
		Path::new(self.relative_path.as_ref())
	}
}

impl From<IsolatedFilePathData<'static>> for file_path::UniqueWhereParam {
	fn from(path: IsolatedFilePathData<'static>) -> Self {
		Self::LocationIdMaterializedPathNameExtensionEquals(
			path.location_id,
			path.materialized_path.into_owned(),
			path.name.into_owned(),
			path.extension.into_owned(),
		)
	}
}

impl From<IsolatedFilePathData<'static>> for file_path::WhereParam {
	fn from(path: IsolatedFilePathData<'static>) -> Self {
		Self::And(vec![
			file_path::location_id::equals(Some(path.location_id)),
			file_path::materialized_path::equals(Some(path.materialized_path.into_owned())),
			file_path::name::equals(Some(path.name.into_owned())),
			file_path::extension::equals(Some(path.extension.into_owned())),
		])
	}
}

impl From<&IsolatedFilePathData<'_>> for file_path::UniqueWhereParam {
	fn from(path: &IsolatedFilePathData<'_>) -> Self {
		Self::LocationIdMaterializedPathNameExtensionEquals(
			path.location_id,
			path.materialized_path.to_string(),
			path.name.to_string(),
			path.extension.to_string(),
		)
	}
}

impl From<&IsolatedFilePathData<'_>> for file_path::WhereParam {
	fn from(path: &IsolatedFilePathData<'_>) -> Self {
		Self::And(vec![
			file_path::location_id::equals(Some(path.location_id)),
			file_path::materialized_path::equals(Some(path.materialized_path.to_string())),
			file_path::name::equals(Some(path.name.to_string())),
			file_path::extension::equals(Some(path.extension.to_string())),
		])
	}
}

impl fmt::Display for IsolatedFilePathData<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.relative_path)
	}
}

impl fmt::Display for IsolatedFilePathDataParts<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.relative_path)
	}
}

#[macro_use]
mod macros {
	macro_rules! impl_from_db {
		($($file_path_kind:ident),+ $(,)?) => {
			$(
				impl ::std::convert::TryFrom<$file_path_kind::Data> for $crate::
					isolated_file_path_data::
					IsolatedFilePathData<'static>
				{
                    type Error = ::sd_utils::db::MissingFieldError;

					fn try_from(path: $file_path_kind::Data) -> Result<Self, Self::Error> {
                        use ::sd_utils::db::maybe_missing;
                        use ::std::borrow::Cow;

                        Ok(Self::from_db_data(
                            maybe_missing(path.location_id, "file_path.location_id")?,
                            maybe_missing(path.is_dir, "file_path.is_dir")?,
                            Cow::Owned(maybe_missing(path.materialized_path, "file_path.materialized_path")?),
                            Cow::Owned(maybe_missing(path.name, "file_path.name")?),
                            Cow::Owned(maybe_missing(path.extension, "file_path.extension")?)
                        ))
					}
				}

				impl<'a> ::std::convert::TryFrom<&'a $file_path_kind::Data> for $crate::
					isolated_file_path_data::
					IsolatedFilePathData<'a>
				{
                    type Error =::sd_utils::db::MissingFieldError;

					fn try_from(path: &'a $file_path_kind::Data) -> Result<Self, Self::Error> {
                        use ::sd_utils::db::maybe_missing;
                        use ::std::borrow::Cow;

						Ok(Self::from_db_data(
							maybe_missing(path.location_id, "file_path.location_id")?,
                            maybe_missing(path.is_dir, "file_path.is_dir")?,
							Cow::Borrowed(maybe_missing(&path.materialized_path, "file_path.materialized_path")?),
							Cow::Borrowed(maybe_missing(&path.name, "file_path.name")?),
							Cow::Borrowed(maybe_missing(&path.extension, "file_path.extension")?)
						))
					}
				}
			)+
		};
	}

	macro_rules! impl_from_db_without_location_id {
		($($file_path_kind:ident),+ $(,)?) => {
			$(
				impl ::std::convert::TryFrom<(::sd_prisma::prisma::location::id::Type, $file_path_kind::Data)> for $crate::
					isolated_file_path_data::
					IsolatedFilePathData<'static>
				{
                    type Error = ::sd_utils::db::MissingFieldError;

					fn try_from((location_id, path): (::sd_prisma::prisma::location::id::Type, $file_path_kind::Data)) -> Result<Self, Self::Error> {
                        use ::sd_utils::db::maybe_missing;
                        use ::std::borrow::Cow;

                        Ok(Self::from_db_data(
                            location_id,
                            maybe_missing(path.is_dir, "file_path.is_dir")?,
                            Cow::Owned(maybe_missing(path.materialized_path, "file_path.materialized_path")?),
                            Cow::Owned(maybe_missing(path.name, "file_path.name")?),
                            Cow::Owned(maybe_missing(path.extension, "file_path.extension")?)
                        ))
					}
				}

				impl<'a> ::std::convert::TryFrom<(::sd_prisma::prisma::location::id::Type, &'a $file_path_kind::Data)> for $crate::
					isolated_file_path_data::
					IsolatedFilePathData<'a>
				{
                    type Error = ::sd_utils::db::MissingFieldError;

					fn try_from((location_id, path): (::sd_prisma::prisma::location::id::Type, &'a $file_path_kind::Data)) -> Result<Self, Self::Error> {
                        use ::sd_utils::db::maybe_missing;
                        use ::std::borrow::Cow;

						Ok(Self::from_db_data(
							location_id,
                            maybe_missing(path.is_dir, "file_path.is_dir")?,
							Cow::Borrowed(maybe_missing(&path.materialized_path, "file_path.materialized_path")?),
							Cow::Borrowed(maybe_missing(&path.name, "file_path.name")?),
							Cow::Borrowed(maybe_missing(&path.extension, "file_path.extension")?)
						))
					}
				}
			)+
		};
	}
}

impl_from_db!(
	file_path,
	file_path_to_isolate,
	file_path_to_isolate_with_pub_id,
	file_path_walker,
	file_path_to_isolate_with_id,
	file_path_with_object,
	file_path_watcher_remove
);

impl_from_db_without_location_id!(
	file_path_for_file_identifier,
	file_path_to_full_path,
	file_path_for_media_processor,
	file_path_for_object_validator,
	file_path_to_handle_custom_uri,
	file_path_to_handle_p2p_serve_file
);

fn extract_relative_path(
	location_id: location::id::Type,
	location_path: impl AsRef<Path>,
	path: impl AsRef<Path>,
) -> Result<String, FilePathError> {
	let path = path.as_ref();

	path.strip_prefix(location_path)
		.map_err(|_| FilePathError::UnableToExtractMaterializedPath {
			location_id,
			path: path.into(),
		})
		.and_then(|relative| {
			relative
				.to_str()
				.map(|relative_str| relative_str.replace('\\', "/"))
				.ok_or_else(|| NonUtf8PathError(path.into()).into())
		})
}

/// This function separates a file path from a location path, and normalizes replacing '\' with '/'
/// to be consistent between Windows and Unix like systems
pub fn extract_normalized_materialized_path_str(
	location_id: location::id::Type,
	location_path: impl AsRef<Path>,
	path: impl AsRef<Path>,
) -> Result<String, FilePathError> {
	let path = path.as_ref();

	path.strip_prefix(location_path)
		.map_err(|_| FilePathError::UnableToExtractMaterializedPath {
			location_id,
			path: path.into(),
		})?
		.parent()
		.map_or_else(
			|| Ok("/".to_string()),
			|materialized_path| {
				materialized_path
					.to_str()
					.map(|materialized_path_str| {
						if materialized_path_str.is_empty() {
							"/".to_string()
						} else {
							format!("/{}/", materialized_path_str.replace('\\', "/"))
						}
					})
					.ok_or_else(|| NonUtf8PathError(path.into()))
			},
		)
		.map_err(Into::into)
}

fn assemble_relative_path(
	materialized_path: &str,
	name: &str,
	extension: &str,
	is_dir: bool,
) -> String {
	if !is_dir && !extension.is_empty() {
		format!("{}{}.{}", &materialized_path[1..], name, extension)
	} else {
		format!("{}{}", &materialized_path[1..], name)
	}
}

pub fn join_location_relative_path(
	location_path: impl AsRef<Path>,
	relative_path: impl AsRef<Path>,
) -> PathBuf {
	let relative_path = relative_path.as_ref();
	let relative_path = relative_path
		.strip_prefix(MAIN_SEPARATOR_STR)
		.unwrap_or(relative_path);

	location_path.as_ref().join(relative_path)
}

pub fn push_location_relative_path(
	mut location_path: PathBuf,
	relative_path: impl AsRef<Path>,
) -> PathBuf {
	let relative_path = relative_path.as_ref();

	let relative_path = relative_path
		.strip_prefix(MAIN_SEPARATOR_STR)
		.unwrap_or(relative_path);
	location_path.push(relative_path);

	location_path
}

#[cfg(test)]
mod tests {
	use super::*;

	fn expected(
		materialized_path: &'static str,
		is_dir: bool,
		name: &'static str,
		extension: &'static str,
		relative_path: &'static str,
	) -> IsolatedFilePathData<'static> {
		IsolatedFilePathData {
			location_id: 1,
			materialized_path: materialized_path.into(),
			is_dir,
			name: name.into(),
			extension: extension.into(),
			relative_path: relative_path.into(),
		}
	}

	#[test]
	fn new_method() {
		let tester = |full_path, is_dir, expected, msg| {
			let actual =
				IsolatedFilePathData::new(1, "/spacedrive/location", full_path, is_dir).unwrap();
			assert_eq!(actual, expected, "{msg}");
		};

		tester(
			"/spacedrive/location",
			true,
			expected("/", true, "", "", ""),
			"the location root directory",
		);

		tester(
			"/spacedrive/location/file.txt",
			false,
			expected("/", false, "file", "txt", "file.txt"),
			"a file in the root directory",
		);

		tester(
			"/spacedrive/location/dir",
			true,
			expected("/", true, "dir", "", "dir"),
			"a directory in the root directory",
		);

		tester(
			"/spacedrive/location/dir/file.txt",
			false,
			expected("/dir/", false, "file", "txt", "dir/file.txt"),
			"a directory with a file inside",
		);

		tester(
			"/spacedrive/location/dir/dir2",
			true,
			expected("/dir/", true, "dir2", "", "dir/dir2"),
			"a directory in a directory",
		);

		tester(
			"/spacedrive/location/dir/dir2/dir3",
			true,
			expected("/dir/dir2/", true, "dir3", "", "dir/dir2/dir3"),
			"3 level of directories",
		);

		tester(
			"/spacedrive/location/dir/dir2/dir3/file.txt",
			false,
			expected(
				"/dir/dir2/dir3/",
				false,
				"file",
				"txt",
				"dir/dir2/dir3/file.txt",
			),
			"a file inside a third level directory",
		);
	}

	#[test]
	fn parent_method() {
		let tester = |full_path, is_dir, expected, msg| {
			let child =
				IsolatedFilePathData::new(1, "/spacedrive/location", full_path, is_dir).unwrap();

			let actual = child.parent();
			assert_eq!(actual, expected, "{msg}");
		};

		tester(
			"/spacedrive/location",
			true,
			expected("/", true, "", "", ""),
			"the location root directory",
		);

		tester(
			"/spacedrive/location/file.txt",
			false,
			expected("/", true, "", "", ""),
			"a file in the root directory",
		);

		tester(
			"/spacedrive/location/dir",
			true,
			expected("/", true, "", "", ""),
			"a directory in the root directory",
		);

		tester(
			"/spacedrive/location/dir/file.txt",
			false,
			expected("/", true, "dir", "", "dir"),
			"a directory with a file inside",
		);

		tester(
			"/spacedrive/location/dir/dir2",
			true,
			expected("/", true, "dir", "", "dir"),
			"a directory in a directory",
		);

		tester(
			"/spacedrive/location/dir/dir2/dir3",
			true,
			expected("/dir/", true, "dir2", "", "dir/dir2"),
			"3 level of directories",
		);

		tester(
			"/spacedrive/location/dir/dir2/dir3/file.txt",
			false,
			expected("/dir/dir2/", true, "dir3", "", "dir/dir2/dir3"),
			"a file inside a third level directory",
		);
	}

	#[test]
	fn extract_normalized_materialized_path() {
		let tester = |path, expected, msg| {
			let actual =
				extract_normalized_materialized_path_str(1, "/spacedrive/location", path).unwrap();
			assert_eq!(actual, expected, "{msg}");
		};

		tester("/spacedrive/location", "/", "the location root directory");
		tester(
			"/spacedrive/location/file.txt",
			"/",
			"a file in the root directory",
		);
		tester(
			"/spacedrive/location/dir",
			"/",
			"a directory in the root directory",
		);
		tester(
			"/spacedrive/location/dir/file.txt",
			"/dir/",
			"a directory with a file inside",
		);
		tester(
			"/spacedrive/location/dir/dir2",
			"/dir/",
			"a directory in a directory",
		);
		tester(
			"/spacedrive/location/dir/dir2/dir3",
			"/dir/dir2/",
			"3 level of directories",
		);
		tester(
			"/spacedrive/location/dir/dir2/dir3/file.txt",
			"/dir/dir2/dir3/",
			"a file inside a third level directory",
		);
	}
}
