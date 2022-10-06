/// Object Kind
///
/// https://www.garykessler.net/library/file_sigs.html
/// https://github.com/bojand/infer/
///
use std::{
	fmt::{Display, Formatter},
	str::FromStr,
};

use int_enum::IntEnum;
use rspc::Type;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Type, IntEnum)]
#[repr(u8)]
pub enum ObjectKind {
	// A file that can not be identified by the indexer
	Unknown = 0,
	// A known filetype, but without specific support
	Document = 1,
	// A virtual filesystem directory
	Folder = 2,
	// A file that contains human-readable text
	Text = 3,
	// A virtual directory int
	Package = 4,
	// An image file
	Image = 5,
	// An audio file
	Audio = 6,
	// A video file
	Video = 7,
	// A compressed archive of data
	Archive = 8,
	// An executable, program or application
	Executable = 9,
	// A link to another object
	Alias = 10,
	// Raw bytes encrypted by Spacedrive with self contained metadata
	Encrypted = 11,
	// A link can open web pages, apps or Spaces
	Key = 12,
	// A link can open web pages, apps or Spaces
	Link = 13,
	// A special filetype that represents a preserved webpage
	WebPageArchive = 14,
	// A widget is a mini app that can be placed in a Space at various sizes, associated Widget struct required
	Widget = 15,
	// Albums can only have one level of children, and are associated with the Album struct
	Album = 16,
	// Its like a folder, but appears like a stack of files, designed for burst photos / associated groups of files
	Collection = 17,
	// You know, text init
	Font = 18,
	// 3D Object
	Mesh = 19,
	// Editable source code file
	Code = 20,
	// Database file
	Database = 21,
}

/// Construct the extensions enum
macro_rules! extension_enum {
	(
		Extension {
			$( $variant:ident($type:ident), )*
		}
	) => {
		// construct enum
		#[derive(Debug, Serialize, Deserialize, PartialEq)]
		pub enum Extension {
			$( $variant($type), )*
		}
		impl Extension {
			pub fn from_str(s: &str) -> Option<ExtensionPossibility> {
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
		impl From<Extension> for ObjectKind {
			fn from(ext: Extension) -> Self {
				match ext {
					$( Extension::$variant(_) => ObjectKind::$variant, )*
				}
			}
		}
		//
		impl Display for Extension {
			fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
				match self {
					$( Extension::$variant(x) =>  write!(f, "{}", x), )*
				}
			}
		}
	};
}

pub fn verify_magic_bytes<T: MagicBytes>(ext: T, file: &mut std::fs::File) -> Option<T> {
	use std::io::{Read, Seek, SeekFrom};

	let mut buf = vec![0; ext.magic_bytes_len()];

	file.seek(SeekFrom::Start(
		ext.magic_bytes_offset().try_into().unwrap(),
	))
	.unwrap();

	file.read_exact(&mut buf).unwrap();
	println!("MAGIC BYTES: {:x?}", buf);

	T::from_magic_bytes_buf(&buf).map(|_| ext)
}

impl Extension {
	pub fn resolve_conflicting(
		ext_str: &str,
		file: &mut std::fs::File,
		always_check_magic_bytes: bool,
	) -> Option<Extension> {
		let ext = match Extension::from_str(ext_str) {
			Some(e) => e,
			None => return None,
		};

		match ext {
			// we don't need to check the magic bytes unless there is conflict
			// always_check_magic_bytes forces the check for tests
			ExtensionPossibility::Known(e) => {
				if always_check_magic_bytes {
					match e {
						Self::Image(x) => verify_magic_bytes(x, file).map(Self::Image),
						Self::Audio(x) => verify_magic_bytes(x, file).map(Self::Audio),
						Self::Video(x) => verify_magic_bytes(x, file).map(Self::Video),
						Self::Executable(x) => verify_magic_bytes(x, file).map(Self::Executable),
						_ => return None,
					}
				} else {
					Some(Extension::from(e))
				}
			}
			ExtensionPossibility::Conflicts(ext) => match ext_str {
				"ts" => {
					let maybe_video_ext = if ext.iter().any(|e| matches!(e, Extension::Video(_))) {
						verify_magic_bytes(VideoExtension::Ts, file).map(Extension::Video)
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

extension_enum! {
	Extension {
		Video(VideoExtension),
		Image(ImageExtension),
		Audio(AudioExtension),
		Archive(ArchiveExtension),
		Executable(ExecutableExtension),
		Text(TextExtension),
		Encrypted(EncryptedExtension),
		Key(KeyExtension),
		Font(FontExtension),
		Mesh(MeshExtension),
		Code(CodeExtension),
		Database(DatabaseExtension),

	}
}

#[derive(Debug, PartialEq)]
pub enum ExtensionPossibility {
	Known(Extension),
	Conflicts(Vec<Extension>),
}

pub trait MagicBytes: Sized {
	fn from_magic_bytes_buf(buf: &[u8]) -> Option<Self>;
	fn magic_bytes_len(&self) -> usize;
	fn magic_bytes_offset(&self) -> usize;
}

macro_rules! magic_byte_value {
	(_) => {
		0 as u8
	};
	($val:literal) => {{
		$val as u8
	}};
}

macro_rules! magic_byte_offset {
	() => {
		0
	};
	($val:literal) => {
		$val
	};
}

/// Define a public enum with static array of all possible variants
/// including implementations to convert to/from string
macro_rules! extension_category_enum {
	(
		$(#[$enum_attr:meta])*
		$enum_name:ident $static_array_name:ident {
			$($(#[$variant_attr:meta])* $variant:ident $(= [$($magic_bytes:tt),*] $(+ $offset:literal)?)? ,)*
		}
	) => {
		#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
		#[serde(rename_all = "snake_case")]
		$(#[$enum_attr])*
		// construct enum
		pub enum $enum_name {
			$( $(#[$variant_attr])* $variant, )*
		}
		// a static array of all variants
		pub static $static_array_name: &[$enum_name] = &[
			$( $enum_name::$variant, )*
		];
		extension_category_enum!(@magic_bytes; $enum_name ( $($(#[$variant_attr])* $variant $(= [$($magic_bytes),*] $(+ $offset)?)? ),* ));
		// convert from string
		impl FromStr for $enum_name {
			type Err = serde_json::Error;
			fn from_str(s: &str) -> Result<Self, Self::Err> {
				serde_json::from_value(Value::String(s.to_string()))
			}
		}
		// convert to string
		impl std::fmt::Display for $enum_name {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				write!(f, "{}", serde_json::to_string(self).unwrap()) // SAFETY: This is safe
			}
		}
	};
	(@magic_bytes; $enum_name:ident ($($(#[$variant_attr:meta])* $variant:ident = [$($magic_bytes:tt),*] $(+ $offset:literal)? ),*)) => {
		impl MagicBytes for $enum_name {
			fn from_magic_bytes_buf(buf: &[u8]) -> Option<Self> {
				match buf {
					$( &[$($magic_bytes),*] => Some($enum_name::$variant),)*
					_ => None
				}
			}
			fn magic_bytes_len(&self) -> usize {
				match self {
					$( $enum_name::$variant => (&[$(magic_byte_value!($magic_bytes)),*] as &[u8]).len() ),*,
				}
			}

			fn magic_bytes_offset(&self) -> usize {
				match self {
					$( $enum_name::$variant => magic_byte_offset!($($offset)?)),*
				}
			}
		}
	};
	(@magic_bytes; $enum_name:ident ($($(#[$variant_attr:meta])* $variant:ident),*)) => {};
}

// video extensions
extension_category_enum! {
	VideoExtension ALL_VIDEO_EXTENSIONS {
		Avi = [0x52, 0x49, 0x46, 0x46, _, _, _, _, 0x41, 0x56, 0x49, 0x20],
		Ts = [0x47],
		Qt = [0x71, 0x74, 0x20, 0x20],
		Mov = [0x66, 0x74, 0x79, 0x70, 0x71, 0x74, 0x20, 0x20] + 4,
		Swf = [0x5A, 0x57, 0x53],
		Mjpeg = [],
		Mpeg = [0x47],
		Mts = [0x47, 0x41, 0x39, 0x34],
		Mxf = [0x06, 0x0E, 0x2B, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0D, 0x01, 0x02, 0x01, 0x01, 0x02],
		M2v = [0x00, 0x00, 0x01, 0xBA],
		Mpg = [],
		Mpe = [],
		M2ts = [],
		Flv = [0x46, 0x4C, 0x56],
		Wm = [],
		#[serde(rename = "3gp")]
		_3gp = [],
		M4v = [0x66, 0x74, 0x79, 0x70, 0x4D, 0x34, 0x56] + 4,
		Wmv = [0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C],
		Asf = [0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C],
		Wma = [0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C],
		Mp4 = [],
		Webm = [0x1A, 0x45, 0xDF, 0xA3],
		Mkv = [0x1A, 0x45, 0xDF, 0xA3],
	}
}

// image extensions
extension_category_enum! {
	ImageExtension _ALL_IMAGE_EXTENSIONS {
		Jpg = [0xFF, 0xD8],
		Jpeg = [0xFF, 0xD8],
		Png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
		Gif = [0x47, 0x49, 0x46, 0x38, _, 0x61],
		Bmp = [0x42, 0x4D],
		Tiff = [0x49, 0x49, 0x2A, 0x00],
		Webp = [0x52, 0x49, 0x46, 0x46, _, _, _, _, 0x57, 0x45, 0x42, 0x50],
		Svg = [0x3C, 0x73, 0x76, 0x67],
		Ico = [0x00, 0x00, 0x01, 0x00],
		Heic = [0x00, 0x00, 0x00, 0x18, 0x66, 0x74, 0x79, 0x70, 0x68, 0x65, 0x69, 0x63],
	}
}

// audio extensions
extension_category_enum! {
	AudioExtension _ALL_AUDIO_EXTENSIONS {
		Mp3 = [0x49, 0x44, 0x33],
		M4a = [0x66, 0x74, 0x79, 0x70, 0x4D, 0x34, 0x41, 0x20] + 4,
		Wav = [0x52, 0x49, 0x46, 0x46, _, _, _, _, 0x57, 0x41, 0x56, 0x45],
		Aiff = [0x46, 0x4F, 0x52, 0x4D, _, _, _, _, 0x41, 0x49, 0x46, 0x46],
		Aif = [0x46, 0x4F, 0x52, 0x4D, _, _, _, _, 0x41, 0x49, 0x46, 0x46],
		Flac = [0x66, 0x4C, 0x61, 0x43],
		Ogg = [0x4F, 0x67, 0x67, 0x53],
		Opus = [0x4F, 0x70, 0x75, 0x73, 0x48, 0x65, 0x61, 0x64],
		Wma = [0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C],
		Amr = [0x23, 0x21, 0x41, 0x4D, 0x52],
	}
}

// archive extensions
extension_category_enum! {
	ArchiveExtension _ALL_ARCHIVE_EXTENSIONS {
		Zip = [0x50, 0x4B, 0x03, 0x04],
		Rar = [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00],
		Tar = [0x75, 0x73, 0x74, 0x61, 0x72],
		Gz = [0x1F, 0x8B, 0x08],
		Bz2 = [0x42, 0x5A, 0x68],
		#[serde(rename = "7z")]
		_7z = [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C],
		Xz = [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00],
	}
}

// executable extensions
extension_category_enum! {
	ExecutableExtension _ALL_EXECUTABLE_EXTENSIONS {
		Exe = [0x4D, 0x5A],
		App = [0x4D, 0x5A],
		Apk = [0x50, 0x4B, 0x03, 0x04],
		Deb = [0x21, 0x3C, 0x61, 0x72, 0x63, 0x68, 0x3E, 0x0A, 0x64, 0x65, 0x62, 0x69, 0x61, 0x6E, 0x2D, 0x62, 0x69, 0x6E, 0x61, 0x72, 0x79],
		Dmg = [0x78, 0x01, 0x73, 0x0D, 0x62, 0x62, 0x60],
		Pkg = [0x4D, 0x5A],
		Rpm = [0xED, 0xAB, 0xEE, 0xDB],
		Msi = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1],
		Jar = [0x50, 0x4B, 0x03, 0x04],
		Bat = [],
	}
}

// document extensions
extension_category_enum! {
	DocumentExtension _ALL_DOCUMENT_EXTENSIONS {
		Pdf = [0x25, 0x50, 0x44, 0x46, 0x2D],
		Key = [0x50, 0x4B, 0x03, 0x04],
		Pages = [0x50, 0x4B, 0x03, 0x04],
		Numbers = [0x50, 0x4B, 0x03, 0x04],
		Doc = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1],
		Docx = [0x50, 0x4B, 0x03, 0x04],
		Xls = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1],
		Xlsx = [0x50, 0x4B, 0x03, 0x04],
		Ppt = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1],
		Pptx = [0x50, 0x4B, 0x03, 0x04],
		Odt = [0x50, 0x4B, 0x03, 0x04],
		Ods = [0x50, 0x4B, 0x03, 0x04],
		Odp = [0x50, 0x4B, 0x03, 0x04],
		Ics = [0x42, 0x45, 0x47, 0x49, 0x4E, 0x3A, 0x56, 0x43, 0x41, 0x52, 0x44],
		Hwp = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1],
	}
}

// text file extensions
extension_category_enum! {
	TextExtension _ALL_TEXT_EXTENSIONS {
		Txt,
		Rtf,
		Md,
		Json,
		Yaml,
		Yml,
		Toml,
		Xml,
		Csv,
		Cfg,
	}
}
// encrypted file extensions
extension_category_enum! {
	EncryptedExtension _ALL_ENCRYPTED_EXTENSIONS {
		// Spacedrive encrypted file
		Bit = [0x73, 0x64, 0x62, 0x69, 0x74],
		// Spacedrive container
		Box = [0x73, 0x64, 0x62, 0x6F, 0x78],
		// Spacedrive block storage,
		Block = [0x73, 0x64, 0x62, 0x6C, 0x6F, 0x63, 0x6B],
	}
}

// key extensions
extension_category_enum! {
	KeyExtension _ALL_KEY_EXTENSIONS {
		Pgp,
		Pub,
		Pem,
		P12,
		P8,
		Keychain,
	}
}

// font extensions
extension_category_enum! {
	FontExtension _ALL_FONT_EXTENSIONS {
		Ttf = [0x00, 0x01, 0x00, 0x00, 0x00],
		Otf = [0x4F, 0x54, 0x54, 0x4F, 0x00],
	}
}

// font extensions
extension_category_enum! {
	MeshExtension _ALL_MESH_EXTENSIONS {
		Fbx = [0x46, 0x42, 0x58, 0x20],
		Obj = [0x6F, 0x62, 0x6A],
	}
}

// code extensions
extension_category_enum! {
	CodeExtension _ALL_CODE_EXTENSIONS {
		Rs,
		Ts,
		Tsx,
		Js,
		Jsx,
		Vue,
		Php,
		Py,
		Rb,
		Sh,
		Html,
		Css,
		Sass,
		Scss,
		Less,
		Bash,
		Zsh,
		C,
		Cpp,
		H,
		Hpp,
		Java,
		Scala,
		Go,
		Dart,
		Swift,
		Mdx,
		Astro,
	}
}

// database extensions
extension_category_enum! {
	DatabaseExtension _ALL_DATABASE_EXTENSIONS {
		Sqlite = [0x53, 0x51, 0x4C, 0x69, 0x74, 0x65, 0x20, 0x66, 0x6F, 0x72, 0x6D, 0x61, 0x74, 0x20, 0x33, 0x00],
	}
}

#[cfg(test)]
mod test {
	use std::path::{Path, PathBuf};

	use super::*;

	#[test]
	fn extension_from_str() {
		// single extension match
		assert_eq!(
			Extension::from_str("jpg"),
			Some(ExtensionPossibility::Known(Extension::Image(
				ImageExtension::Jpg
			)))
		);
		// with conflicts
		assert_eq!(
			Extension::from_str("ts"),
			Some(ExtensionPossibility::Conflicts(vec![
				Extension::Video(VideoExtension::Ts),
				Extension::Code(CodeExtension::Ts)
			]))
		);
		// invalid case
		assert_eq!(Extension::from_str("jeff"), None);
	}
	#[test]
	fn magic_bytes() {
		fn test_path(subpath: &str) -> Option<Extension> {
			let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
				.parent()
				.unwrap()
				.join("packages/test-files/files")
				.join(subpath);
			let mut file = std::fs::File::open(path).unwrap();
			Extension::resolve_conflicting(&subpath.split(".").last().unwrap(), &mut file, true)
		}

		assert_eq!(
			dbg!(test_path("video/video.ts")),
			Some(Extension::Video(VideoExtension::Ts))
		);
		assert_eq!(
			dbg!(test_path("code/typescript.ts")),
			Some(Extension::Code(CodeExtension::Ts))
		);
		assert_eq!(
			dbg!(test_path("video/video.3gp")),
			Some(Extension::Video(VideoExtension::_3gp))
		);
		assert_eq!(
			dbg!(test_path("video/video.mov")),
			Some(Extension::Video(VideoExtension::Mov))
		);
		assert_eq!(
			dbg!(test_path("video/video.asf")),
			Some(Extension::Video(VideoExtension::Asf))
		);
		assert_eq!(
			dbg!(test_path("video/video.avi")),
			Some(Extension::Video(VideoExtension::Avi))
		);
		assert_eq!(
			dbg!(test_path("video/video.flv")),
			Some(Extension::Video(VideoExtension::Flv))
		);
		assert_eq!(
			dbg!(test_path("video/video.m4v")),
			Some(Extension::Video(VideoExtension::M4v))
		);
		assert_eq!(
			dbg!(test_path("video/video.mkv")),
			Some(Extension::Video(VideoExtension::Mkv))
		);
		assert_eq!(
			dbg!(test_path("video/video.mpg")),
			Some(Extension::Video(VideoExtension::Mpg))
		);
		assert_eq!(
			dbg!(test_path("video/video.mpeg")),
			Some(Extension::Video(VideoExtension::Mpeg))
		);
		assert_eq!(
			dbg!(test_path("video/video.mts")),
			Some(Extension::Video(VideoExtension::Mts))
		);
		assert_eq!(
			dbg!(test_path("video/video.mxf")),
			Some(Extension::Video(VideoExtension::Mxf))
		);
	}
}
