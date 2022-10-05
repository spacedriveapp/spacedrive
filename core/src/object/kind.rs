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

use crate::prisma::file_path;

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

#[cfg(test)]
mod test {
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
				Extension::Text(TextExtension::Ts)
			]))
		);
		// invalid case
		assert_eq!(Extension::from_str("jeff"), None);
	}
}

impl file_path::Data {
	// fn extension(&self, magic_bytes: Option<&[u8]>) -> Option<ExtensionPossibility>V {
	// 	let ext = match self.extension {
	// 		Some(ext) => ext.as_str(),
	// 		None => return Ok(Extension::Unknown("".to_string())),
	// 	};

	// 	// if let Ok(ex) = VideoExtension::from_str(ext) {
	// 	// 	// .ts files can be video or text
	// 	// 	match ex {
	// 	// 		VideoExtension::Ts => {
	// 	// 			// double check if it is a video=
	// 	// 		}
	// 	// 		_ => Extension::Video(ex),
	// 	// 	}
	// 	// //
	// 	// } else if let Ok(ex) = ImageExtension::from_str(ext) {
	// 	// 	return Extension::Image(ex);
	// 	// //
	// 	// } else if let Ok(ex) = AudioExtension::from_str(ext) {
	// 	// 	return Extension::Audio(ex);
	// 	// //
	// 	// } else {
	// 	// 	return Extension::Unknown(ext);
	// 	// }
	// }

	// fn object_kind(&self, magic_bytes: Option<&[u8]>) -> ObjectKind {
	// 	let extension = self.extension(magic_bytes);
	// 	extension.into()
	// }
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
		Mpeg = [0x47],
		Mts = [0x47, 0x41, 0x39, 0x34],
		Mpg = [],
		Mpe = [],
		Qt = [0x71, 0x74, 0x20, 0x20],
		Mov = [0x66, 0x74, 0x79, 0x70, 0x71, 0x74, 0x20, 0x20] + 4,
		Swf = [0x5A, 0x57, 0x53],
		Mjpeg = [],
		Ts = [0x47],
		Mxf = [0x06, 0x0E, 0x2B, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0D, 0x01, 0x02, 0x01, 0x01, 0x02],
		M2v = [0x00, 0x00, 0x01, 0xBA],
		M2ts = [],
		Flv = [0x66, 0x74, 0x79, 0x70, 0x4D, 0x34, 0x56, 0x20] + 4,
		Wm = [],
		#[serde(rename = "3gp")]
		_3gp = [],
		M4v = [0x66, 0x74, 0x79, 0x70, 0x6D, 0x70, 0x34, 0x32] + 4,
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
		Exe = [],
		App = [],
		Apk = [0x50, 0x4B, 0x03, 0x04],
		Deb = [],
		Dmg = [],
		Pkg = [],
		Rpm = [],
		Msi = [],
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
	}
}

// text file extensions
extension_category_enum! {
	TextExtension _ALL_TEXT_EXTENSIONS {
		Txt,
		Rtf,
		Csv,
		Html,
		Css,
		Json,
		Yaml,
		Xml,
		Md,
		Ts,
	}
}
// encrypted file extensions
extension_category_enum! {
	EncryptedExtension _ALL_ENCRYPTED_EXTENSIONS {
		Bit,
		Box,
		Block,
	}
}
// Spacedrive encrypted file
// Spacedrive container
// Spacedrive block storage,

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
