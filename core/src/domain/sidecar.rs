//! What a derived artifact is, and how it is named.
//!
//! Kind, variant and format are the vocabulary a sidecar is addressed by, and
//! they are what `SdPath::Sidecar` parses. They sit in the domain because they
//! outlive whatever writes them: the same three name a thumbnail in the hot
//! tier and a transcript in the durable one.

use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
	fmt,
	path::{Path, PathBuf},
};
use uuid::Uuid;

/// What a sidecar holds. The core kinds are closed; an extension declares
/// its own in its manifest (`write_sidecars`) and they live under the
/// extension's namespace, so two extensions' `faces` never collide and no
/// extension can write a core kind.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SidecarKind {
	Thumb,
	Thumbstrip,
	Proxy,
	Embeddings,
	Ocr,
	Transcript,
	GaussianSplat,
	Audio,
	Shots,
	Keyframe,
	Extension { extension_id: String, kind: String },
}

impl SidecarKind {
	/// A kind an extension declared. Both parts must be single path
	/// segments of `[A-Za-z0-9._-]`, since they become directory names.
	pub fn extension(extension_id: &str, kind: &str) -> Result<Self, String> {
		for (what, value) in [("extension id", extension_id), ("kind", kind)] {
			let safe = !value.is_empty()
				&& value != "."
				&& value != ".."
				&& value
					.chars()
					.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
			if !safe {
				return Err(format!("Invalid sidecar {what}: {value:?}"));
			}
		}
		Ok(Self::Extension {
			extension_id: extension_id.to_string(),
			kind: kind.to_string(),
		})
	}

	pub fn as_str(&self) -> &str {
		match self {
			Self::Thumb => "thumb",
			Self::Thumbstrip => "thumbstrip",
			Self::Proxy => "proxy",
			Self::Embeddings => "embeddings",
			Self::Ocr => "ocr",
			Self::Transcript => "transcript",
			Self::GaussianSplat => "gaussian_splat",
			Self::Audio => "audio",
			Self::Shots => "shots",
			Self::Keyframe => "keyframe",
			Self::Extension { kind, .. } => kind,
		}
	}

	/// The directory under a content's sidecar directory, relative to the
	/// `sidecars` tree root.
	pub fn directory(&self) -> String {
		// The layout crate owns the mapping so out-of-process readers resolve
		// the same directories; every core variant is a known kind string.
		match self {
			Self::Extension { extension_id, kind } => {
				sd_sidecar_path::extension_kind_directory(extension_id, kind)
			}
			core => sd_sidecar_path::kind_directory(core.as_str())
				.expect("every core SidecarKind maps to a directory")
				.to_string(),
		}
	}
}

impl fmt::Display for SidecarKind {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl TryFrom<&str> for SidecarKind {
	type Error = String;

	fn try_from(value: &str) -> Result<Self, Self::Error> {
		match value {
			"thumb" => Ok(Self::Thumb),
			"thumbstrip" => Ok(Self::Thumbstrip),
			"proxy" => Ok(Self::Proxy),
			"embeddings" => Ok(Self::Embeddings),
			"ocr" => Ok(Self::Ocr),
			"transcript" => Ok(Self::Transcript),
			"gaussian_splat" => Ok(Self::GaussianSplat),
			"audio" => Ok(Self::Audio),
			"shots" => Ok(Self::Shots),
			"keyframe" => Ok(Self::Keyframe),
			_ => Err(format!("Invalid sidecar kind: {}", value)),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub struct SidecarVariant(pub String);

impl SidecarVariant {
	pub fn new(variant: impl Into<String>) -> Self {
		Self(variant.into())
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}
}

impl fmt::Display for SidecarVariant {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl From<&str> for SidecarVariant {
	fn from(value: &str) -> Self {
		Self(value.to_string())
	}
}

impl From<String> for SidecarVariant {
	fn from(value: String) -> Self {
		Self(value)
	}
}

/// Format for storing sidecar files
///
/// Format selection guidelines:
/// - Webp: Thumbnails and image derivatives (compressed images)
/// - Mp4: Video/audio proxies (standard media format)
/// - Json: Text-based structured data (OCR, transcripts)
/// - MessagePack: Binary structured data (embeddings, vectors)
/// - Text: Plain text extractions
/// - Ply: 3D model format for Gaussian splats
/// - Wav: Uncompressed audio that speech models read without decoding
/// - Srt, Vtt: Subtitles that editors and players import as they are
/// - Jpeg: Video keyframes, which FFmpeg writes without optional encoders
///
/// MessagePack is preferred for embeddings because:
/// - 6x smaller than JSON (1.7KB vs 10KB per 384-dim vector)
/// - 10x faster to parse
/// - Already used in Spacedrive (job serialization)
/// - Enables sub-30ms semantic search on 1M+ files
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SidecarFormat {
	Webp,
	Mp4,
	Json,
	MessagePack,
	Text,
	Ply,
	Wav,
	Srt,
	Vtt,
	Jpeg,
}

impl SidecarFormat {
	pub fn extension(&self) -> &'static str {
		match self {
			Self::Webp => "webp",
			Self::Mp4 => "mp4",
			Self::Json => "json",
			Self::MessagePack => "msgpack",
			Self::Text => "txt",
			Self::Ply => "ply",
			Self::Wav => "wav",
			Self::Srt => "srt",
			Self::Vtt => "vtt",
			Self::Jpeg => "jpg",
		}
	}

	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Webp => "webp",
			Self::Mp4 => "mp4",
			Self::Json => "json",
			Self::MessagePack => "messagepack",
			Self::Text => "text",
			Self::Ply => "ply",
			Self::Wav => "wav",
			Self::Srt => "srt",
			Self::Vtt => "vtt",
			Self::Jpeg => "jpeg",
		}
	}
}

impl fmt::Display for SidecarFormat {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl TryFrom<&str> for SidecarFormat {
	type Error = String;

	fn try_from(value: &str) -> Result<Self, Self::Error> {
		match value {
			"webp" => Ok(Self::Webp),
			"mp4" => Ok(Self::Mp4),
			"json" => Ok(Self::Json),
			"msgpack" | "messagepack" => Ok(Self::MessagePack),
			"text" | "txt" => Ok(Self::Text),
			"ply" => Ok(Self::Ply),
			"wav" => Ok(Self::Wav),
			"srt" => Ok(Self::Srt),
			"vtt" => Ok(Self::Vtt),
			"jpeg" | "jpg" => Ok(Self::Jpeg),
			_ => Err(format!("Invalid sidecar format: {}", value)),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidecarStatus {
	Pending,
	Ready,
	Failed,
}

impl SidecarStatus {
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Pending => "pending",
			Self::Ready => "ready",
			Self::Failed => "failed",
		}
	}
}

impl fmt::Display for SidecarStatus {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl TryFrom<&str> for SidecarStatus {
	type Error = String;

	fn try_from(value: &str) -> Result<Self, Self::Error> {
		match value {
			"pending" => Ok(Self::Pending),
			"ready" => Ok(Self::Ready),
			"failed" => Ok(Self::Failed),
			_ => Err(format!("Invalid sidecar status: {}", value)),
		}
	}
}

impl TryFrom<String> for SidecarFormat {
	type Error = String;

	fn try_from(value: String) -> Result<Self, Self::Error> {
		Self::try_from(value.as_str())
	}
}

/// Height of the short side of a video proxy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub enum ProxyResolution {
	#[default]
	#[serde(rename = "720p")]
	P720,
	#[serde(rename = "1080p")]
	P1080,
}

impl ProxyResolution {
	pub fn short_side(&self) -> u32 {
		match self {
			Self::P720 => 720,
			Self::P1080 => 1080,
		}
	}

	pub fn variant(&self) -> &'static str {
		match self {
			Self::P720 => "720p",
			Self::P1080 => "1080p",
		}
	}
}

/// A sidecar addressed by the bytes it was derived from.
///
/// Media sidecars follow content rather than a record, so a moved, renamed
/// or copied file finds the proxy and audio already made for its bytes. The
/// content uuid is the candidate one, `sd_store::content::uuid_for` of the
/// sampled hash: it exists before a full hash does and stays put when one
/// lands. Every writer and reader resolves paths here, so the layout in
/// `docs/core/virtual-sidecars.mdx` has one implementation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentSidecar {
	pub content_uuid: Uuid,
	pub kind: SidecarKind,
	pub variant: SidecarVariant,
	pub format: SidecarFormat,
}

impl ContentSidecar {
	/// Speech audio: 16 kHz mono 16-bit PCM WAV, the input speech models take.
	pub const SPEECH_AUDIO_VARIANT: &'static str = "16k_mono";

	/// A transcript; the model that made it is recorded inside the JSON.
	pub const TRANSCRIPT_VARIANT: &'static str = "default";

	/// H.264 proxy with a keyframe every second.
	pub fn proxy(content_uuid: Uuid, resolution: ProxyResolution) -> Self {
		Self {
			content_uuid,
			kind: SidecarKind::Proxy,
			variant: SidecarVariant::new(resolution.variant()),
			format: SidecarFormat::Mp4,
		}
	}

	pub fn speech_audio(content_uuid: Uuid) -> Self {
		Self {
			content_uuid,
			kind: SidecarKind::Audio,
			variant: SidecarVariant::new(Self::SPEECH_AUDIO_VARIANT),
			format: SidecarFormat::Wav,
		}
	}

	/// The transcript of a recording. One variant in three formats: segment
	/// JSON (`Json`, what search and the core read), and `Srt` and `Vtt`
	/// for editors and players.
	pub fn transcript(content_uuid: Uuid, format: SidecarFormat) -> Self {
		Self {
			content_uuid,
			kind: SidecarKind::Transcript,
			variant: SidecarVariant::new(Self::TRANSCRIPT_VARIANT),
			format,
		}
	}

	/// The shot list the shot detection job writes.
	pub fn shots(content_uuid: Uuid) -> Self {
		Self {
			content_uuid,
			kind: SidecarKind::Shots,
			variant: SidecarVariant::new("default"),
			format: SidecarFormat::Json,
		}
	}

	/// The representative frame of one shot, named by the shot id.
	pub fn keyframe(content_uuid: Uuid, shot_id: Uuid) -> Self {
		Self {
			content_uuid,
			kind: SidecarKind::Keyframe,
			variant: SidecarVariant::new(shot_id.to_string()),
			format: SidecarFormat::Jpeg,
		}
	}

	/// Path relative to a derived root, `sources/<id>/derived`.
	pub fn relative_path(&self) -> PathBuf {
		sd_sidecar_path::relative_path(
			&self.content_uuid,
			&self.kind.directory(),
			self.variant.as_str(),
			self.format.extension(),
		)
	}

	/// `.<variant>.partial.<ext>` beside the sidecar under `root`, the name
	/// a writer fills before publishing. Readers skip dot files, so a
	/// half-written one is never taken for the sidecar.
	pub fn partial_path(&self, root: &Path) -> PathBuf {
		root.join(self.relative_path()).with_file_name(format!(
			".{}.partial.{}",
			self.variant.as_str(),
			self.format.extension()
		))
	}

	/// `core` for the core's kinds, the extension's id for one it declared.
	pub fn producer(&self) -> &str {
		match &self.kind {
			SidecarKind::Extension { extension_id, .. } => extension_id,
			_ => CORE_PRODUCER,
		}
	}

	/// The key its row is stored under.
	pub fn key(&self) -> sd_store::ContentSidecarKey {
		sd_store::ContentSidecarKey {
			content_uuid: self.content_uuid,
			producer: self.producer().to_string(),
			kind: self.kind.as_str().to_string(),
			variant: self.variant.as_str().to_string(),
			format: self.format.extension().to_string(),
		}
	}

	/// The sidecar a row lists, or `None` for one this build cannot name.
	pub fn from_key(key: &sd_store::ContentSidecarKey) -> Option<Self> {
		let kind = if key.producer == CORE_PRODUCER {
			SidecarKind::try_from(key.kind.as_str()).ok()?
		} else {
			SidecarKind::extension(&key.producer, &key.kind).ok()?
		};
		Some(Self {
			content_uuid: key.content_uuid,
			kind,
			variant: SidecarVariant::new(key.variant.clone()),
			format: SidecarFormat::try_from(key.format.as_str()).ok()?,
		})
	}

	/// The sidecar a path relative to a derived root names, the inverse of
	/// [`Self::relative_path`]. `None` for anything else in the tree,
	/// including a writer's partial file.
	pub fn from_relative_path(path: &Path) -> Option<Self> {
		let parts: Vec<&str> = path
			.components()
			.map(|part| part.as_os_str().to_str())
			.collect::<Option<_>>()?;
		let (content_uuid, kind, file) = match parts.as_slice() {
			[_, _, uuid, "extensions", extension_id, kind, file] => {
				(uuid, SidecarKind::extension(extension_id, kind).ok()?, file)
			}
			[_, _, uuid, directory, file] => (
				uuid,
				SidecarKind::try_from(sd_sidecar_path::directory_kind(directory)?).ok()?,
				file,
			),
			_ => return None,
		};
		if file.starts_with('.') {
			return None;
		}
		let (variant, extension) = file.rsplit_once('.')?;
		let sidecar = Self {
			content_uuid: Uuid::parse_str(content_uuid).ok()?,
			kind,
			variant: SidecarVariant::new(variant),
			format: SidecarFormat::try_from(extension).ok()?,
		};
		// One name per sidecar: `clip.jpeg` is not where a keyframe lives.
		(sidecar.relative_path() == path).then_some(sidecar)
	}
}

/// The producer named for artifacts the core makes.
pub const CORE_PRODUCER: &str = "core";

/// The candidate content uuid of a local file, the key media sidecars use.
///
/// This is the same sampled hash the indexer stores, so a file the indexer
/// has reached and one it has not resolve to the same sidecars.
pub async fn content_uuid_of_file(
	path: &Path,
) -> Result<Uuid, crate::domain::content_identity::ContentHashError> {
	let hash =
		crate::domain::content_identity::ContentHashGenerator::generate_content_hash(path).await?;
	Ok(sd_store::content::uuid_for(&hash))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn media_sidecars_resolve_to_the_documented_layout() {
		let uuid = Uuid::parse_str("abcd1234-5678-90ab-cdef-123456789012").unwrap();
		assert_eq!(
			ContentSidecar::proxy(uuid, ProxyResolution::P720).relative_path(),
			PathBuf::from("ab/cd/abcd1234-5678-90ab-cdef-123456789012/proxies/720p.mp4")
		);
	}

	#[test]
	fn a_relative_path_names_its_sidecar_and_nothing_else() {
		let uuid = Uuid::parse_str("abcd1234-5678-90ab-cdef-123456789012").unwrap();
		let shot = Uuid::new_v4();
		for sidecar in [
			ContentSidecar::proxy(uuid, ProxyResolution::P720),
			ContentSidecar::transcript(uuid, SidecarFormat::Vtt),
			ContentSidecar::keyframe(uuid, shot),
			ContentSidecar {
				content_uuid: uuid,
				kind: SidecarKind::extension("com.spacedrive.photos", "faces").unwrap(),
				variant: SidecarVariant::new("default"),
				format: SidecarFormat::Json,
			},
		] {
			assert_eq!(
				ContentSidecar::from_relative_path(&sidecar.relative_path()),
				Some(sidecar.clone())
			);
			assert_eq!(ContentSidecar::from_key(&sidecar.key()), Some(sidecar));
		}
		let partial = ContentSidecar::shots(uuid).partial_path(Path::new(""));
		assert_eq!(ContentSidecar::from_relative_path(&partial), None);
		assert_eq!(
			ContentSidecar::from_relative_path(Path::new(
				"ab/cd/abcd1234-5678-90ab-cdef-123456789012/keyframes/x.jpeg"
			)),
			None
		);
	}

	#[test]
	fn proxy_resolution_serializes_as_its_variant() {
		assert_eq!(
			serde_json::to_string(&ProxyResolution::P1080).unwrap(),
			"\"1080p\""
		);
		assert_eq!(
			serde_json::from_str::<ProxyResolution>("\"720p\"").unwrap(),
			ProxyResolution::P720
		);
	}
}
