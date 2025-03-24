///
/// References:
/// https://www.garykessler.net/library/file_sigs.html
/// https://github.com/bojand/infer/
/// https://github.com/features/copilot
///
use crate::magic::{
	extension_category_enum, extension_enum, ExtensionPossibility, MagicBytes, MagicBytesMeta,
};

extension_enum! {
	Extension {
		Document(DocumentExtension),
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
		Book(BookExtension),
		Config(ConfigExtension),
	}
}

// video extensions
extension_category_enum! {
	VideoExtension ALL_VIDEO_EXTENSIONS {
		Avi = [0x52, 0x49, 0x46, 0x46, _, _, _, _, 0x41, 0x56, 0x49, 0x20],
		Avifs = [],
		Qt = [0x71, 0x74, 0x20, 0x20],
		Mov = [0x66, 0x74, 0x79, 0x70, 0x71, 0x74, 0x20, 0x20] + 4,
		Swf = [0x5A, 0x57, 0x53] | [0x46, 0x57, 0x53],
		Mjpeg = [],
		Ts = [0x47],
		Mts = [0x47] | [_, _, _, 0x47],
		Mpeg = [0x47] | [0x00, 0x00, 0x01, 0xBA] | [0x00, 0x00, 0x01, 0xB3],
		Mxf = [0x06, 0x0E, 0x2B, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0D, 0x01, 0x02, 0x01, 0x01, 0x02],
		M2v = [0x00, 0x00, 0x01, 0xBA],
		Mpg = [],
		Mpe = [],
		M2ts = [],
		Flv = [0x46, 0x4C, 0x56],
		Wm = [],
		#[serde(rename = "3gp")]
		#[strum(serialize = "3gp")]
		_3gp = [],
		M4v = [0x66, 0x74, 0x79, 0x70, 0x4D, 0x34, 0x56] + 4,
		Wmv = [0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C],
		Asf = [0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C],
		Mp4 = [],
		Webm = [0x1A, 0x45, 0xDF, 0xA3],
		Mkv = [0x1A, 0x45, 0xDF, 0xA3],
		Vob = [0x00, 0x00, 0x01, 0xBA],
		Ogv = [0x4F, 0x67, 0x67, 0x53],
		Wtv = [0xB7, 0xD8, 0x00],
		Hevc = [],
		F4v = [0x66, 0x74, 0x79, 0x70, 0x66, 0x72, 0x65, 0x65] + 4,
	}
}

// image extensions
extension_category_enum! {
	ImageExtension ALL_IMAGE_EXTENSIONS {
		Jpg = [0xFF, 0xD8],
		Jpeg = [0xFF, 0xD8],
		Png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
		Apng = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52],
		Gif = [0x47, 0x49, 0x46, 0x38, _, 0x61],
		Bmp = [0x42, 0x4D],
		Tiff = [0x49, 0x49, 0x2A, 0x00],
		Webp = [0x52, 0x49, 0x46, 0x46, _, _, _, _, 0x57, 0x45, 0x42, 0x50],
		Svg = [0x3C, 0x73, 0x76, 0x67],
		Ico = [0x00, 0x00, 0x01, 0x00],
		Heic = [0x00, 0x00, 0x00, 0x18, 0x66, 0x74, 0x79, 0x70, 0x68, 0x65, 0x69, 0x63],
		Heics  = [0x00, 0x00, 0x00, 0x18, 0x66, 0x74, 0x79, 0x70, 0x68, 0x65, 0x69, 0x63],
		Heif = [],
		Heifs = [],
		Hif = [],
		Avif = [],
		Avci = [],
		Avcs = [],
		Raw = [],
		Akw = [0x41, 0x4B, 0x57, 0x42],
		Dng = [0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00, 0x44, 0x4E, 0x47, 0x00],
		Cr2 = [0x49, 0x49, 0x2A, 0x00, 0x10, 0x00, 0x00, 0x00, 0x43, 0x52, 0x02, 0x00],
		Dcr = [0x49, 0x49, 0x2A, 0x00, 0x10, 0x00, 0x00, 0x00, 0x44, 0x43, 0x52, 0x00],
		Nwr = [0x49, 0x49, 0x2A, 0x00, 0x10, 0x00, 0x00, 0x00, 0x4E, 0x57, 0x52, 0x00],
		Nef = [0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00, 0x4E, 0x45, 0x46, 0x00],
		Arw = [0x49, 0x49, 0x2A, 0x00, 0x08],
		Rw2 = [0x49, 0x49, 0x2A, 0x00, 0x18],
	}
}

// audio extensions
extension_category_enum! {
	AudioExtension ALL_AUDIO_EXTENSIONS {
		Mp3 = [0x49, 0x44, 0x33],
		Mp2 = [0xFF, 0xFB] | [0xFF, 0xFD],
		M4a = [0x66, 0x74, 0x79, 0x70, 0x4D, 0x34, 0x41, 0x20] + 4,
		Wav = [0x52, 0x49, 0x46, 0x46, _, _, _, _, 0x57, 0x41, 0x56, 0x45],
		Aiff = [0x46, 0x4F, 0x52, 0x4D, _, _, _, _, 0x41, 0x49, 0x46, 0x46],
		Aif = [0x46, 0x4F, 0x52, 0x4D, _, _, _, _, 0x41, 0x49, 0x46, 0x46],
		Flac = [0x66, 0x4C, 0x61, 0x43],
		Ogg = [0x4F, 0x67, 0x67, 0x53],
		Oga = [0x4F, 0x67, 0x67, 0x53],
		Opus = [0x4F, 0x70, 0x75, 0x73, 0x48, 0x65, 0x61, 0x64] + 28,
		Wma = [0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C],
		Amr = [0x23, 0x21, 0x41, 0x4D, 0x52],
		Aac = [0xFF, 0xF1],
		Wv = [0x77, 0x76, 0x70, 0x6B],
		Voc = [0x43, 0x72, 0x65, 0x61, 0x74, 0x69, 0x76, 0x65, 0x20, 0x56, 0x6F, 0x69, 0x63, 0x65, 0x20, 0x46, 0x69, 0x6C, 0x65],
		Tta = [0x54, 0x54, 0x41],
		Loas = [0x56, 0xE0],
		Caf = [0x63, 0x61, 0x66, 0x66],
		Aptx = [0x4B, 0xBF, 0x4B, 0xBF],
		Adts = [0xFF, 0xF1],
		Ast = [0x53, 0x54, 0x52, 0x4D],
		Mid = [0x4D, 0x54, 0x68, 0x64],
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
		#[strum(serialize = "7z")]
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
	DocumentExtension ALL_DOCUMENT_EXTENSIONS {
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
		Markdown,
		Csv,
		Tsv,
	}
}
// config file extensions
extension_category_enum! {
	ConfigExtension _ALL_CONFIG_EXTENSIONS {
		Ini,
		Json,
		Yaml,
		Yml,
		Toml,
		Xml,
		Mathml,
		Rss,
		Csv,
		Cfg,
		Compose,
		Tsconfig,
	}
}

// encrypted file extensions
extension_category_enum! {
	EncryptedExtension _ALL_ENCRYPTED_EXTENSIONS {
		// Spacedrive encrypted file
		Bytes = [0x62, 0x61, 0x6C, 0x6C, 0x61, 0x70, 0x70],
		// Spacedrive container
		Container = [0x73, 0x64, 0x62, 0x6F, 0x78],
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
		Woff = [0x77, 0x4F, 0x46, 0x46],
		Woff2 = [0x77, 0x4F, 0x46, 0x32],
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
		// AppleScript
		Scpt,
		Scptd,
		Applescript,
		// Shell script
		Sh,
		Zsh,
		Fish,
		Bash,
		// C, C++
		C,
		Cpp,
		H,
		Hpp,
		// Ruby
		Rb,
		// Javascript
		Js,
		Mjs,
		Jsx,
		// Markup
		Html,
		// Stylesheet
		Css,
		Sass,
		Scss,
		Less,
		// Crystal
		Cr,
		// C#
		Cs,
		Csx,
		D,
		Dart,
		// Docker
		Dockerfile,
		Go,
		// Haskell
		Hs,
		Java,
		// Kotlin
		Kt,
		Kts,
		Lua,
		// Makefile
		Make,
		Nim,
		Nims,
		// Objective-C
		M,
		Mm,
		// Ocaml
		Ml,
		Mli,
		Mll,
		Mly,
		// Perl
		Pl,
		// PHP
		Php,
		Php1,
		Php2,
		Php3,
		Php4,
		Php5,
		Php6,
		Phps,
		Phpt,
		Phtml,
		// Powershell
		Ps1,
		Psd1,
		Psm1,
		// Python
		Py,
		Qml,
		R,
		// Rust
		Rs,
		// Solidity
		Sol,
		Sql,
		Swift,
		// Typescript
		Ts,
		Tsx,
		Vala,
		Zig,
		Vue,
		Scala,
		Mdx,
		Astro,
		Mts,
	}
}

// database extensions
extension_category_enum! {
	DatabaseExtension _ALL_DATABASE_EXTENSIONS {
		Sqlite = [0x53, 0x51, 0x4C, 0x69, 0x74, 0x65, 0x20, 0x66, 0x6F, 0x72, 0x6D, 0x61, 0x74, 0x20, 0x33, 0x00],
		Db = [],
	}
}

// book extensions
extension_category_enum! {
	BookExtension _ALL_BOOK_EXTENSIONS {
		Azw = [0x52, 0x49, 0x46, 0x46],
		Azw3 = [0x52, 0x49, 0x46, 0x46],
		Epub = [0x50, 0x4B, 0x03, 0x04],
		Mobi = [0x4D, 0x4F, 0x42, 0x49],
	}
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
				Extension::Code(CodeExtension::Ts)
			]))
		);
		// invalid case
		assert_eq!(Extension::from_str("jeff"), None);
	}

	#[tokio::test]
	async fn magic_bytes() {
		async fn test_path(subpath: &str) -> Option<Extension> {
			println!("testing {subpath}...");
			Extension::resolve_conflicting(subpath.split('.').last().unwrap(), true).await
		}
		// Video extension tests
		assert_eq!(
			dbg!(test_path("video/video.ts").await),
			Some(Extension::Video(VideoExtension::Ts))
		);
		assert_eq!(
			dbg!(test_path("code/typescript.ts").await),
			Some(Extension::Code(CodeExtension::Ts))
		);
		assert_eq!(
			dbg!(test_path("video/video.3gp").await),
			Some(Extension::Video(VideoExtension::_3gp))
		);
		assert_eq!(
			dbg!(test_path("video/video.mov").await),
			Some(Extension::Video(VideoExtension::Mov))
		);
		assert_eq!(
			dbg!(test_path("video/video.asf").await),
			Some(Extension::Video(VideoExtension::Asf))
		);
		assert_eq!(
			dbg!(test_path("video/video.avi").await),
			Some(Extension::Video(VideoExtension::Avi))
		);
		assert_eq!(
			dbg!(test_path("video/video.flv").await),
			Some(Extension::Video(VideoExtension::Flv))
		);
		assert_eq!(
			dbg!(test_path("video/video.m4v").await),
			Some(Extension::Video(VideoExtension::M4v))
		);
		assert_eq!(
			dbg!(test_path("video/video.mkv").await),
			Some(Extension::Video(VideoExtension::Mkv))
		);
		assert_eq!(
			dbg!(test_path("video/video.mpg").await),
			Some(Extension::Video(VideoExtension::Mpg))
		);
		assert_eq!(
			dbg!(test_path("video/video.mpeg").await),
			Some(Extension::Video(VideoExtension::Mpeg))
		);
		assert_eq!(
			dbg!(test_path("video/video.mts").await),
			Some(Extension::Video(VideoExtension::Mts))
		);
		assert_eq!(
			dbg!(test_path("video/video.mxf").await),
			Some(Extension::Video(VideoExtension::Mxf))
		);
		assert_eq!(
			dbg!(test_path("video/video.ogv").await),
			Some(Extension::Video(VideoExtension::Ogv))
		);
		assert_eq!(
			dbg!(test_path("video/video.swf").await),
			Some(Extension::Video(VideoExtension::Swf))
		);
		assert_eq!(
			dbg!(test_path("video/video.ts").await),
			Some(Extension::Video(VideoExtension::Ts))
		);
		assert_eq!(
			dbg!(test_path("video/video.vob").await),
			Some(Extension::Video(VideoExtension::Vob))
		);
		assert_eq!(
			dbg!(test_path("video/video.ogv").await),
			Some(Extension::Video(VideoExtension::Ogv))
		);
		assert_eq!(
			dbg!(test_path("video/video.wmv").await),
			Some(Extension::Video(VideoExtension::Wmv))
		);
		assert_eq!(
			dbg!(test_path("video/video.wtv").await),
			Some(Extension::Video(VideoExtension::Wtv))
		);

		// Audio extension tests
		assert_eq!(
			dbg!(test_path("audio/audio.aac").await),
			Some(Extension::Audio(AudioExtension::Aac))
		);
		assert_eq!(
			dbg!(test_path("audio/audio.adts").await),
			Some(Extension::Audio(AudioExtension::Adts))
		);
		assert_eq!(
			dbg!(test_path("audio/audio.aif").await),
			Some(Extension::Audio(AudioExtension::Aif))
		);
		assert_eq!(
			dbg!(test_path("audio/audio.aiff").await),
			Some(Extension::Audio(AudioExtension::Aiff))
		);
		assert_eq!(
			dbg!(test_path("audio/audio.aptx").await),
			Some(Extension::Audio(AudioExtension::Aptx))
		);
		assert_eq!(
			dbg!(test_path("audio/audio.ast").await),
			Some(Extension::Audio(AudioExtension::Ast))
		);
		assert_eq!(
			dbg!(test_path("audio/audio.caf").await),
			Some(Extension::Audio(AudioExtension::Caf))
		);
		assert_eq!(
			dbg!(test_path("audio/audio.flac").await),
			Some(Extension::Audio(AudioExtension::Flac))
		);
		assert_eq!(
			dbg!(test_path("audio/audio.loas").await),
			Some(Extension::Audio(AudioExtension::Loas))
		);
		assert_eq!(
			dbg!(test_path("audio/audio.m4a").await),
			Some(Extension::Audio(AudioExtension::M4a))
		);
		// assert_eq!(
		// 	dbg!(test_path("audio/audio.m4b").await),
		// 	Some(Extension::Audio(AudioExtension::M4b))
		// );
		assert_eq!(
			dbg!(test_path("audio/audio.mp2").await),
			Some(Extension::Audio(AudioExtension::Mp2))
		);
		assert_eq!(
			dbg!(test_path("audio/audio.mp3").await),
			Some(Extension::Audio(AudioExtension::Mp3))
		);
		assert_eq!(
			dbg!(test_path("audio/audio.oga").await),
			Some(Extension::Audio(AudioExtension::Oga))
		);
		assert_eq!(
			dbg!(test_path("audio/audio.ogg").await),
			Some(Extension::Audio(AudioExtension::Ogg))
		);
		assert_eq!(
			dbg!(test_path("audio/audio.opus").await),
			Some(Extension::Audio(AudioExtension::Opus))
		);
		assert_eq!(
			dbg!(test_path("audio/audio.tta").await),
			Some(Extension::Audio(AudioExtension::Tta))
		);
		assert_eq!(
			dbg!(test_path("audio/audio.voc").await),
			Some(Extension::Audio(AudioExtension::Voc))
		);
		assert_eq!(
			dbg!(test_path("audio/audio.wav").await),
			Some(Extension::Audio(AudioExtension::Wav))
		);
		assert_eq!(
			dbg!(test_path("audio/audio.wma").await),
			Some(Extension::Audio(AudioExtension::Wma))
		);
		assert_eq!(
			dbg!(test_path("audio/audio.wv").await),
			Some(Extension::Audio(AudioExtension::Wv))
		);
	}
}
