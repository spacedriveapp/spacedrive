#![allow(dead_code)]

use crate::extensions::{CodeExtension, Extension, VideoExtension};
use std::{ffi::OsStr, io::SeekFrom, path::Path};

use tokio::{
	fs::{self, File},
	io::{AsyncReadExt, AsyncSeekExt},
};

#[derive(Debug, PartialEq, Eq)]
pub enum ExtensionPossibility {
	Known(Extension),
	Conflicts(Vec<Extension>),
}

#[derive(Debug)]
pub struct MagicBytesMeta {
	pub offset: usize,
	pub length: usize,
}

pub trait MagicBytes: Sized + PartialEq {
	fn has_magic_bytes(&self, buf: &[u8]) -> bool;
	fn magic_bytes_meta(&self) -> Vec<MagicBytesMeta>;
}

#[macro_export]
macro_rules! magic_byte_value {
	(_) => {
		0 as u8
	};
	($val:literal) => {{
		$val as u8
	}};
}
// pub(crate) use magic_byte_value;

#[macro_export]
macro_rules! magic_byte_offset {
	() => {
		0
	};
	($val:literal) => {
		$val
	};
}
// pub(crate) use magic_byte_offset;

macro_rules! extension_enum {
	(
		Extension {
			$( $variant:ident($type:ident), )*
		}
	) => {
		// construct enum
		#[derive(Debug, ::serde::Serialize, ::serde::Deserialize, PartialEq, Eq)]
		pub enum Extension {
			$( $variant($type), )*
		}
		impl Extension {
			#[allow(clippy::should_implement_trait)]
			pub fn from_str(s: &str) -> Option<ExtensionPossibility> {
				use std::str::FromStr;
				let mut exts = [$(
						$type::from_str(s).ok().map(Self::$variant)
					),*]
					.into_iter()
					.filter_map(|s| s)
					.collect::<Vec<_>>();

				match exts {
					_ if exts.len() == 0 => None,
					_ if exts.len() == 1 => Some(ExtensionPossibility::Known(exts.swap_remove(0))),
					_ => Some(ExtensionPossibility::Conflicts(exts))
				}
			}
		}
		// convert Extension to ObjectKind
		impl From<Extension> for $crate::kind::ObjectKind {
			fn from(ext: Extension) -> Self {
				match ext {
					$( Extension::$variant(_) => $crate::kind::ObjectKind::$variant, )*
				}
			}
		}
		//
		impl std::fmt::Display for Extension {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				match self {
					$( Extension::$variant(x) =>  write!(f, "{}", x), )*
				}
			}
		}
	};
}
pub(crate) use extension_enum;

/// Define a public enum with static array of all possible variants
/// including implementations to convert to/from string
macro_rules! extension_category_enum {
	(
		$(#[$enum_attr:meta])*
		$enum_name:ident $static_array_name:ident {
			$($(#[$variant_attr:meta])* $variant:ident $(= $( [$($magic_bytes:tt),*] $(+ $offset:literal)? )|+ )? ,)*
		}
	) => {
		#[derive(Debug, ::serde::Serialize, ::serde::Deserialize, ::strum::Display, Clone, Copy, PartialEq, Eq)]
		#[serde(rename_all = "snake_case")]
		#[strum(serialize_all = "snake_case")]
		$(#[$enum_attr])*

		// construct enum
		pub enum $enum_name {
			$( $(#[$variant_attr])* $variant, )*
		}
		// a static array of all variants
		pub static $static_array_name: &[$enum_name] = &[
			$( $enum_name::$variant, )*
		];

		$crate::magic::extension_category_enum!(@magic_bytes; $enum_name ( $($(#[$variant_attr])* $variant $(= $( [$($magic_bytes),*] $(+ $offset)? )|+ )? ),* ));

		// convert from string
		impl std::str::FromStr for $enum_name {
			type Err = serde_json::Error;
			fn from_str(s: &str) -> Result<Self, Self::Err> {
				serde_json::from_value(serde_json::Value::String(s.to_string()))
			}
		}
	};

	(@magic_bytes; $enum_name:ident ($($(#[$variant_attr:meta])* $variant:ident = $( [$($magic_bytes:tt),*] $(+ $offset:literal)? )|+ ),*)) => {
		impl MagicBytes for $enum_name {
			fn has_magic_bytes(&self, buf: &[u8]) -> bool {
				match (self, buf) {
					$( $( ($enum_name::$variant, &[$($magic_bytes,)* ..]) => true, )+ )*
					_ => false
				}
			}
			// get offset and length of magic bytes
			fn magic_bytes_meta(&self) -> Vec<MagicBytesMeta> {
				match self {
					$( $enum_name::$variant => vec![
						$( MagicBytesMeta {
							length: (&[$($crate::magic_byte_value!($magic_bytes)),*] as &[u8]).len(),
							offset: $crate::magic_byte_offset!($($offset)?),
						}, )+
					] ),*
				}
			}
		}
	};
	(@magic_bytes; $enum_name:ident ($($(#[$variant_attr:meta])* $variant:ident),*)) => {};
}
pub(crate) use extension_category_enum;

pub async fn verify_magic_bytes<T: MagicBytes>(ext: T, file: &mut File) -> Option<T> {
	for magic in ext.magic_bytes_meta() {
		let mut buf = vec![0; magic.length];

		file.seek(SeekFrom::Start(magic.offset as u64)).await.ok()?;
		file.read_exact(&mut buf).await.ok()?;

		if ext.has_magic_bytes(&buf) {
			return Some(ext);
		}
	}

	None
}

impl Extension {
	pub async fn resolve_conflicting(
		path: impl AsRef<Path>,
		always_check_magic_bytes: bool,
	) -> Option<Extension> {
		let Some(ext_str) = path.as_ref().extension().and_then(OsStr::to_str) else {
            return None
        };

		let Some(ext) = Extension::from_str(ext_str) else {
			return None
		};

		let Ok(ref mut file) = File::open(&path).await else {
            return None
        };

		match ext {
			// we don't need to check the magic bytes unless there is conflict
			// always_check_magic_bytes forces the check for tests
			ExtensionPossibility::Known(e) => {
				if always_check_magic_bytes {
					match e {
						Self::Image(x) => verify_magic_bytes(x, file).await.map(Self::Image),
						Self::Audio(x) => verify_magic_bytes(x, file).await.map(Self::Audio),
						Self::Video(x) => verify_magic_bytes(x, file).await.map(Self::Video),
						Self::Archive(x) => verify_magic_bytes(x, file).await.map(Self::Archive),
						Self::Executable(x) => {
							verify_magic_bytes(x, file).await.map(Self::Executable)
						}
						Self::Font(x) => verify_magic_bytes(x, file).await.map(Self::Font),
						Self::Encrypted(x) => {
							verify_magic_bytes(x, file).await.map(Self::Encrypted)
						}
						Self::Mesh(x) => verify_magic_bytes(x, file).await.map(Self::Mesh),
						Self::Database(x) => verify_magic_bytes(x, file).await.map(Self::Database),
						_ => Some(e),
					}
				} else {
					Some(e)
				}
			}
			ExtensionPossibility::Conflicts(ext) => match ext_str {
				"ts" => {
					let maybe_video_ext = if ext.iter().any(|e| matches!(e, Extension::Video(_))) {
						verify_magic_bytes(VideoExtension::Ts, file)
							.await
							.map(Extension::Video)
					} else {
						None
					};
					Some(maybe_video_ext.unwrap_or(Extension::Code(CodeExtension::Ts)))
				}
				_ => None,
			},
		}
	}
}
