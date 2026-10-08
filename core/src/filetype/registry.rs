//! File type registry - the main API for file type identification

use super::kinds::{ExtensionKind, KindConflict, EXTENSION_KIND_PRIORITY};
use super::{FileType, FileTypeError, IdentificationMethod, IdentificationResult, Result};
use crate::domain::ContentKind;
use crate::filetype::magic::MagicBytePattern;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// Maximum bytes to read for magic byte identification
pub const MAX_MAGIC_BYTES: usize = 8192;

/// Maximum bytes to read for content analysis
const MAX_CONTENT_BYTES: usize = 4096;

/// TOML structure for file type definitions
#[derive(Debug, Deserialize)]
struct FileTypeDefinitions {
	file_types: Vec<FileTypeDefinition>,
}

/// TOML structure for a single file type
#[derive(Debug, Deserialize)]
struct FileTypeDefinition {
	id: String,
	name: String,
	extensions: Vec<String>,
	mime_types: Vec<String>,
	#[serde(default)]
	uti: Option<String>,
	category: String,
	priority: u8,
	#[serde(default)]
	magic_bytes: Vec<MagicByteDefinition>,
	#[serde(default)]
	metadata: serde_json::Value,
}

/// TOML structure for magic bytes
#[derive(Debug, Deserialize)]
struct MagicByteDefinition {
	pattern: String,
	offset: usize,
	priority: u8,
}

/// Registry of all known file types
///
/// Two layers exist for the process: the built-in registry, parsed once from
/// the embedded definitions, and the current registry, which is the built-in
/// one plus every loaded extension's kinds. The plugin manager swaps the
/// current one on load and unload; every lookup goes through
/// [`Self::current`] so a kind filter and a preview never disagree about the
/// same file.
#[derive(Clone)]
pub struct FileTypeRegistry {
	/// All registered file types by ID
	types: HashMap<String, FileType>,

	/// Extension to type IDs mapping
	extension_map: HashMap<String, Vec<String>>,

	/// MIME type to type ID mapping
	mime_map: HashMap<String, String>,

	/// Extension kinds whose claim on a file extension lost to a kind loaded
	/// earlier, by file extension. Their types stay in `types` so the content
	/// identity phase can still check their magic patterns.
	contested: HashMap<String, Vec<String>>,

	/// Every dropped claim, in load order, for `extensions.list`.
	conflicts: Vec<KindConflict>,

	/// Extension kind ids in load order, so a tie between kinds that
	/// nothing but their magic bytes distinguishes resolves the same way
	/// the extension table does.
	extension_kind_order: Vec<String>,
}

static BUILTIN: OnceLock<Arc<FileTypeRegistry>> = OnceLock::new();
static CURRENT: OnceLock<RwLock<Arc<FileTypeRegistry>>> = OnceLock::new();

impl FileTypeRegistry {
	/// The built-in registry, built once for the process.
	///
	/// Building one parses every built-in definition. That is affordable once
	/// and not once per entry: an arena rebuilt from a delivered database adds
	/// more than a million rows one at a time.
	pub fn builtin() -> &'static Self {
		BUILTIN.get_or_init(|| Arc::new(Self::new()))
	}

	/// The registry every lookup should use: the built-in types plus the
	/// kinds of every loaded extension. The built-in registry alone until an
	/// extension with kinds loads.
	pub fn current() -> Arc<Self> {
		CURRENT
			.get_or_init(|| RwLock::new(BUILTIN.get_or_init(|| Arc::new(Self::new())).clone()))
			.read()
			.unwrap_or_else(|poisoned| poisoned.into_inner())
			.clone()
	}

	/// Replace the current registry with the built-in types plus these
	/// extensions' kinds, in load order. The plugin manager calls this after
	/// every load and unload; an empty list restores the built-in registry.
	pub fn install_current(extensions: &[(String, Vec<ExtensionKind>)]) -> Arc<Self> {
		let registry = Arc::new(Self::with_extension_kinds(extensions));
		let slot = CURRENT
			.get_or_init(|| RwLock::new(BUILTIN.get_or_init(|| Arc::new(Self::new())).clone()));
		*slot
			.write()
			.unwrap_or_else(|poisoned| poisoned.into_inner()) = registry.clone();
		registry
	}

	/// The built-in registry plus these extensions' kinds, in load order.
	pub fn with_extension_kinds(extensions: &[(String, Vec<ExtensionKind>)]) -> Self {
		let mut registry = Self::builtin().clone();
		for (extension_id, kinds) in extensions {
			for kind in kinds {
				registry.register_extension_kind(extension_id, kind);
			}
		}
		registry
	}

	/// Why a kind may not load against the built-in table, or `None`.
	///
	/// A kind may refine a file extension the built-in table maps to its own
	/// parent, or claim one the table does not know. Redefining `.pdf` as an
	/// image would change what every consumer of the parent sees, so that is
	/// refused rather than layered.
	pub fn refusal_for(&self, kind: &ExtensionKind) -> Option<String> {
		for ext in &kind.extensions {
			let builtin = self
				.get_by_extension(ext)
				.into_iter()
				.filter(|ft| ft.kind_name.is_none())
				.max_by_key(|ft| ft.priority)
				.map(|ft| ft.category)
				.unwrap_or(ContentKind::Unknown);
			if builtin != ContentKind::Unknown && builtin != kind.parent {
				return Some(format!(
					"kind {:?} claims .{ext} with parent {} but the built-in table maps it to {}",
					kind.name, kind.parent, builtin
				));
			}
		}
		None
	}

	/// Add one extension kind. A file extension another extension kind
	/// already holds stays with it; this kind's claim is recorded as a
	/// conflict and the type is kept for its magic patterns.
	fn register_extension_kind(&mut self, extension_id: &str, kind: &ExtensionKind) {
		let id = kind.id(extension_id);
		for ext in &kind.extensions {
			let ext = ext.to_lowercase();
			let holder = self
				.extension_map
				.get(&ext)
				.into_iter()
				.flatten()
				.find(|id| self.types.get(*id).is_some_and(|ft| ft.kind_name.is_some()))
				.cloned();
			match holder {
				Some(claimed_by) => {
					self.contested
						.entry(ext.clone())
						.or_default()
						.push(id.clone());
					self.conflicts.push(KindConflict {
						extension: ext,
						kind: id.clone(),
						claimed_by,
					});
				}
				None => self.extension_map.entry(ext).or_default().push(id.clone()),
			}
		}
		for mime in &kind.mime_types {
			self.mime_map
				.entry(mime.clone())
				.or_insert_with(|| id.clone());
		}
		self.extension_kind_order.push(id.clone());
		self.types.insert(
			id.clone(),
			FileType {
				id: id.clone(),
				name: kind
					.display_name
					.clone()
					.unwrap_or_else(|| kind.name.clone()),
				extensions: kind.extensions.clone(),
				mime_types: kind.mime_types.clone(),
				uti: None,
				magic_bytes: kind.magic_patterns(),
				category: kind.parent,
				priority: EXTENSION_KIND_PRIORITY,
				metadata: serde_json::Value::Null,
				kind_name: Some(id),
			},
		);
	}

	/// Every extension kind with the file extensions it holds, contested
	/// claims left out, in no particular order.
	pub fn extension_kinds(&self) -> Vec<(&FileType, Vec<String>)> {
		self.types
			.values()
			.filter(|ft| ft.kind_name.is_some())
			.map(|ft| {
				let held = ft
					.extensions
					.iter()
					.map(|e| e.to_lowercase())
					.filter(|ext| {
						self.extension_map
							.get(ext)
							.is_some_and(|ids| ids.contains(&ft.id))
					})
					.collect();
				(ft, held)
			})
			.collect()
	}

	/// Every dropped claim, in load order.
	pub fn conflicts(&self) -> &[KindConflict] {
		&self.conflicts
	}

	/// The extension kinds whose magic patterns decide a file with this
	/// extension: the kind holding the extension when it declares patterns,
	/// and every contested kind for it. For an extension no type claims,
	/// every extension kind with patterns, since the name says nothing.
	/// Empty means the extension lookup is the whole answer.
	pub fn magic_candidates(&self, extension: Option<&str>) -> Vec<&FileType> {
		let with_magic = |ft: &&FileType| ft.kind_name.is_some() && !ft.magic_bytes.is_empty();
		let Some(ext) = extension.map(str::to_lowercase) else {
			return Vec::new();
		};
		let claimed = self.get_by_extension(&ext);
		if claimed.is_empty() {
			return self
				.extension_kind_order
				.iter()
				.filter_map(|id| self.types.get(id))
				.filter(with_magic)
				.collect();
		}
		let mut candidates: Vec<&FileType> = claimed.into_iter().filter(with_magic).collect();
		candidates.extend(
			self.contested
				.get(&ext)
				.into_iter()
				.flatten()
				.filter_map(|id| self.types.get(id))
				.filter(with_magic),
		);
		candidates
	}

	/// The kind a file's header decides, given the candidates
	/// [`Self::magic_candidates`] named and the type its extension mapped to.
	///
	/// A lone match assigns that kind, which is how a contested kind still
	/// wins the files whose bytes it recognizes. Several matches keep the
	/// extension's holder when it is among them. No match keeps the
	/// extension result.
	pub fn resolve_by_magic<'a>(
		&'a self,
		by_extension: Option<&'a FileType>,
		candidates: &[&'a FileType],
		header: &[u8],
	) -> Option<&'a FileType> {
		let matched: Vec<&FileType> = candidates
			.iter()
			.copied()
			.filter(|ft| ft.magic_bytes.iter().any(|p| p.matches(header)))
			.collect();
		match matched.as_slice() {
			[] => by_extension,
			[only] => Some(only),
			several => by_extension
				.filter(|holder| several.iter().any(|ft| ft.id == holder.id))
				.or(Some(several[0])),
		}
	}

	pub fn new() -> Self {
		let mut registry = Self {
			types: HashMap::new(),
			extension_map: HashMap::new(),
			mime_map: HashMap::new(),
			contested: HashMap::new(),
			conflicts: Vec::new(),
			extension_kind_order: Vec::new(),
		};

		// Load built-in types
		registry.load_builtin_types();

		registry
	}

	/// Load built-in file type definitions
	fn load_builtin_types(&mut self) {
		// Load all TOML definitions from the builtin module
		let toml_definitions = super::builtin::get_builtin_toml_definitions();

		for toml_content in toml_definitions {
			// Use the loader to parse TOML
			if let Err(e) = self.load_from_toml(toml_content) {
				eprintln!("Failed to load builtin definitions: {}", e);
			}
		}
	}

	/// Register a file type
	pub fn register(&mut self, file_type: FileType) -> Result<()> {
		// Add to main registry
		let id = file_type.id.clone();

		// Update extension map
		for ext in &file_type.extensions {
			self.extension_map
				.entry(ext.to_lowercase())
				.or_insert_with(Vec::new)
				.push(id.clone());
		}

		// Update MIME map
		for mime in &file_type.mime_types {
			self.mime_map.insert(mime.clone(), id.clone());
		}

		self.types.insert(id, file_type);

		Ok(())
	}

	/// Get a file type by ID
	pub fn get(&self, id: &str) -> Option<&FileType> {
		self.types.get(id)
	}

	/// Get file types by extension
	pub fn get_by_extension(&self, ext: &str) -> Vec<&FileType> {
		let ext = ext.trim_start_matches('.').to_lowercase();

		self.extension_map
			.get(&ext)
			.map(|ids| ids.iter().filter_map(|id| self.types.get(id)).collect())
			.unwrap_or_default()
	}

	/// Get file type by MIME type
	pub fn get_by_mime(&self, mime: &str) -> Option<&FileType> {
		self.mime_map.get(mime).and_then(|id| self.types.get(id))
	}

	/// Get file types by content category
	pub fn get_by_category(&self, category: ContentKind) -> Vec<&FileType> {
		self.types
			.values()
			.filter(|file_type| file_type.category == category)
			.collect()
	}

	/// Get all extensions for a content category
	pub fn get_extensions_for_category(&self, category: ContentKind) -> Vec<&str> {
		self.get_by_category(category)
			.into_iter()
			.flat_map(|file_type| file_type.extensions.iter().map(|s| s.as_str()))
			.collect()
	}

	/// Fast identification by extension only (no file I/O)
	///
	/// This is useful for quick file type detection during indexing where
	/// we don't need high-confidence identification. Returns the content kind
	/// based purely on extension matching.
	///
	/// Returns `ContentKind::Unknown` if the extension is not recognized.
	pub fn identify_by_extension(&self, path: &Path) -> ContentKind {
		self.type_by_extension(path)
			.map(|ft| ft.category)
			.unwrap_or(ContentKind::Unknown)
	}

	/// The highest priority type the path's extension maps to, which is an
	/// extension kind when one holds the extension and a built-in type
	/// otherwise. `None` for no extension or one nothing claims.
	pub fn type_by_extension(&self, path: &Path) -> Option<&FileType> {
		let extension = path.extension().and_then(|s| s.to_str())?;
		self.get_by_extension(extension)
			.into_iter()
			.max_by_key(|ft| ft.priority)
	}

	/// Identify a file type from a path
	pub async fn identify(&self, path: &Path) -> Result<IdentificationResult> {
		// Get extension
		let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");

		// Get possible types by extension
		let candidates = self.get_by_extension(extension);

		match candidates.len() {
			0 => {
				// No extension match, try magic bytes on all types
				self.identify_by_magic_bytes(path, &self.types.values().collect::<Vec<_>>())
					.await
			}
			1 => {
				// Single match, verify with magic bytes if available
				let file_type = candidates[0];
				if file_type.magic_bytes.is_empty() {
					Ok(IdentificationResult {
						file_type: file_type.clone(),
						confidence: 90,
						method: IdentificationMethod::Extension,
					})
				} else {
					// Verify with magic bytes
					match self.check_magic_bytes(path, file_type).await {
						Ok(true) => Ok(IdentificationResult {
							file_type: file_type.clone(),
							confidence: 100,
							method: IdentificationMethod::Combined,
						}),
						_ => Ok(IdentificationResult {
							file_type: file_type.clone(),
							confidence: 70,
							method: IdentificationMethod::Extension,
						}),
					}
				}
			}
			_ => {
				// Multiple candidates, use magic bytes to resolve
				self.identify_by_magic_bytes(path, &candidates).await
			}
		}
	}

	/// Identify by magic bytes from a set of candidates
	async fn identify_by_magic_bytes(
		&self,
		path: &Path,
		candidates: &[&FileType],
	) -> Result<IdentificationResult> {
		// Read file header
		let mut file = File::open(path).await?;
		let mut buffer = vec![0u8; MAX_MAGIC_BYTES];
		let bytes_read = file.read(&mut buffer).await?;
		buffer.truncate(bytes_read);

		// Check each candidate
		let mut matches: Vec<(&FileType, u8)> = Vec::new();

		for candidate in candidates {
			for pattern in &candidate.magic_bytes {
				if pattern.matches(&buffer) {
					matches.push((candidate, pattern.priority));
					break;
				}
			}
		}

		// Sort by priority (highest first)
		matches.sort_by_key(|(_, priority)| std::cmp::Reverse(*priority));

		if let Some((file_type, _)) = matches.first() {
			Ok(IdentificationResult {
				file_type: (*file_type).clone(),
				confidence: 95,
				method: IdentificationMethod::MagicBytes,
			})
		} else {
			// No magic byte match, try content analysis for text files
			if candidates
				.iter()
				.any(|ft| matches!(ft.category, ContentKind::Text | ContentKind::Code))
			{
				self.identify_by_content(path, candidates).await
			} else {
				Err(FileTypeError::UnknownType)
			}
		}
	}

	/// Check if a specific file type's magic bytes match
	async fn check_magic_bytes(&self, path: &Path, file_type: &FileType) -> Result<bool> {
		if file_type.magic_bytes.is_empty() {
			return Ok(true);
		}

		let mut file = File::open(path).await?;
		let mut buffer = vec![0u8; MAX_MAGIC_BYTES];
		let bytes_read = file.read(&mut buffer).await?;
		buffer.truncate(bytes_read);

		Ok(file_type
			.magic_bytes
			.iter()
			.any(|pattern| pattern.matches(&buffer)))
	}

	/// Identify by content analysis (for text files)
	async fn identify_by_content(
		&self,
		path: &Path,
		candidates: &[&FileType],
	) -> Result<IdentificationResult> {
		// Read first part of file
		let mut file = File::open(path).await?;
		let mut buffer = vec![0u8; MAX_CONTENT_BYTES];
		let bytes_read = file.read(&mut buffer).await?;
		buffer.truncate(bytes_read);

		// Try to convert to string
		if let Ok(content) = String::from_utf8(buffer) {
			// Simple heuristics for now
			if content.contains("import")
				|| content.contains("export")
				|| content.contains("interface")
			{
				// Likely TypeScript
				if let Some(ts) = candidates.iter().find(|ft| ft.id == "text/typescript") {
					return Ok(IdentificationResult {
						file_type: (*ts).clone(),
						confidence: 85,
						method: IdentificationMethod::ContentAnalysis,
					});
				}
			}
		}

		// Default to first text candidate
		if let Some(text_type) = candidates
			.iter()
			.find(|ft| matches!(ft.category, ContentKind::Text | ContentKind::Code))
		{
			Ok(IdentificationResult {
				file_type: (*text_type).clone(),
				confidence: 60,
				method: IdentificationMethod::Extension,
			})
		} else {
			Err(FileTypeError::UnknownType)
		}
	}

	/// Load definitions from a TOML string
	pub fn load_from_toml(&mut self, content: &str) -> Result<()> {
		let defs: FileTypeDefinitions = toml::from_str(content)
			.map_err(|e| FileTypeError::InvalidConfig(format!("TOML parse error: {}", e)))?;

		for def in defs.file_types {
			let file_type = self.definition_to_file_type(def)?;
			self.register(file_type)?;
		}

		Ok(())
	}

	/// Convert a definition to a FileType
	fn definition_to_file_type(&self, def: FileTypeDefinition) -> Result<FileType> {
		// Parse category
		let category = match def.category.as_str() {
			"document" => ContentKind::Document,
			"video" => ContentKind::Video,
			"image" => ContentKind::Image,
			"audio" => ContentKind::Audio,
			"archive" => ContentKind::Archive,
			"executable" => ContentKind::Executable,
			"text" => ContentKind::Text,
			"code" => ContentKind::Code,
			"database" => ContentKind::Database,
			"book" => ContentKind::Book,
			"font" => ContentKind::Font,
			"mesh" => ContentKind::Mesh,
			"config" => ContentKind::Config,
			"encrypted" => ContentKind::Encrypted,
			"key" => ContentKind::Key,
			"spreadsheet" => ContentKind::Spreadsheet,
			"presentation" => ContentKind::Presentation,
			"email" => ContentKind::Email,
			"calendar" => ContentKind::Calendar,
			"contact" => ContentKind::Contact,
			"web" => ContentKind::Web,
			"shortcut" => ContentKind::Shortcut,
			"package" => ContentKind::Package,
			_ => ContentKind::Unknown,
		};

		// Parse magic bytes
		let mut magic_bytes = Vec::new();
		for mb_def in def.magic_bytes {
			let pattern =
				MagicBytePattern::from_hex_string(&mb_def.pattern, mb_def.offset, mb_def.priority)
					.map_err(|e| {
						FileTypeError::InvalidConfig(format!("Invalid magic bytes: {}", e))
					})?;
			magic_bytes.push(pattern);
		}

		Ok(FileType {
			id: def.id,
			name: def.name,
			extensions: def.extensions,
			mime_types: def.mime_types,
			uti: def.uti,
			magic_bytes,
			category,
			priority: def.priority,
			metadata: def.metadata,
			kind_name: None,
		})
	}
}

impl Default for FileTypeRegistry {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_registry_basic() {
		let registry = FileTypeRegistry::new();

		// Test getting by extension
		let jpeg_types = registry.get_by_extension("jpg");
		assert_eq!(jpeg_types.len(), 1);
		assert_eq!(jpeg_types[0].id, "image/jpeg");

		// Test getting by MIME
		let png_type = registry.get_by_mime("image/png");
		assert!(png_type.is_some());
		assert_eq!(png_type.unwrap().id, "image/png");

		// Test extension conflict
		let ts_types = registry.get_by_extension("ts");
		assert_eq!(ts_types.len(), 2); // TypeScript and MPEG-TS
	}

	fn kind(name: &str, parent: ContentKind, extensions: &[&str], magic: &[&str]) -> ExtensionKind {
		ExtensionKind {
			name: name.into(),
			display_name: None,
			parent,
			extensions: extensions.iter().map(|e| e.to_string()).collect(),
			mime_types: Vec::new(),
			magic: magic
				.iter()
				.map(|pattern| super::super::kinds::MagicPatternSpec {
					pattern: pattern.to_string(),
					offset: 0,
				})
				.collect(),
			preview: None,
		}
	}

	#[test]
	fn an_extension_kind_refines_a_built_in_extension_and_goes_away_on_unload() {
		let raw = kind("raw", ContentKind::Image, &["cr2", "dng"], &[]);
		let layered = FileTypeRegistry::with_extension_kinds(&[("photos".to_string(), vec![raw])]);
		let path = Path::new("shot.dng");
		let ft = layered.type_by_extension(path).unwrap();
		assert_eq!(ft.id, "photos:raw");
		assert_eq!(ft.kind_name.as_deref(), Some("photos:raw"));
		assert_eq!(layered.identify_by_extension(path), ContentKind::Image);
		assert_eq!(
			layered.type_by_extension(Path::new("a.jpg")).unwrap().id,
			"image/jpeg",
			"other built-in types are untouched"
		);

		let unloaded = FileTypeRegistry::with_extension_kinds(&[]);
		assert_eq!(
			unloaded.type_by_extension(path).unwrap().id,
			"image/x-adobe-dng"
		);
		assert!(unloaded.conflicts().is_empty());
	}

	#[test]
	fn a_kind_may_not_redefine_a_built_in_extension() {
		let builtin = FileTypeRegistry::builtin();
		let bad = kind("pdf-as-image", ContentKind::Image, &["pdf"], &[]);
		let reason = builtin.refusal_for(&bad).unwrap();
		assert!(reason.contains(".pdf"), "{reason}");
		assert!(reason.contains("document"), "{reason}");

		let refine = kind("raw", ContentKind::Image, &["cr2"], &[]);
		assert_eq!(builtin.refusal_for(&refine), None);
		let fresh = kind("thing", ContentKind::Database, &["zzznoone"], &[]);
		assert_eq!(builtin.refusal_for(&fresh), None);
	}

	#[test]
	fn two_extensions_claiming_one_extension_resolve_by_load_order() {
		let first = kind("fake", ContentKind::Image, &["xyz"], &["46 41 4B 45"]);
		let second = kind(
			"other",
			ContentKind::Image,
			&["xyz", "abc"],
			&["4F 54 48 52"],
		);
		let layered = FileTypeRegistry::with_extension_kinds(&[
			("one".to_string(), vec![first]),
			("two".to_string(), vec![second]),
		]);

		assert_eq!(
			layered.type_by_extension(Path::new("f.xyz")).unwrap().id,
			"one:fake"
		);
		assert_eq!(
			layered.type_by_extension(Path::new("f.abc")).unwrap().id,
			"two:other",
			"an uncontested extension of the loser still loads"
		);
		assert_eq!(
			layered.conflicts(),
			&[KindConflict {
				extension: "xyz".into(),
				kind: "two:other".into(),
				claimed_by: "one:fake".into(),
			}]
		);

		let candidates = layered.magic_candidates(Some("xyz"));
		let ids: Vec<&str> = candidates.iter().map(|ft| ft.id.as_str()).collect();
		assert_eq!(ids, ["one:fake", "two:other"]);
		let holder = layered.type_by_extension(Path::new("f.xyz"));
		assert_eq!(
			layered
				.resolve_by_magic(holder, &candidates, b"OTHR....")
				.unwrap()
				.id,
			"two:other",
			"a lone match on a contested kind wins the file"
		);
		assert_eq!(
			layered
				.resolve_by_magic(holder, &candidates, b"FAKE....")
				.unwrap()
				.id,
			"one:fake"
		);
		assert_eq!(
			layered
				.resolve_by_magic(holder, &candidates, b"????....")
				.unwrap()
				.id,
			"one:fake",
			"no match keeps the extension result"
		);
		assert!(
			layered.magic_candidates(Some("jpg")).is_empty(),
			"a built-in extension nobody refines reads no bytes"
		);
		let unknown: Vec<&str> = layered
			.magic_candidates(Some("zzzunknown"))
			.iter()
			.map(|ft| ft.id.as_str())
			.collect();
		assert_eq!(
			unknown,
			["one:fake", "two:other"],
			"an unknown extension checks every kind with magic, in load order"
		);
		let both = FileTypeRegistry::with_extension_kinds(&[
			(
				"one".to_string(),
				vec![kind("a", ContentKind::Text, &["qqq"], &["4F 54"])],
			),
			(
				"two".to_string(),
				vec![kind("b", ContentKind::Text, &["rrr"], &["4F 54 48"])],
			),
		]);
		let candidates = both.magic_candidates(Some("zzzunknown"));
		assert_eq!(
			both.resolve_by_magic(None, &candidates, b"OTHR")
				.unwrap()
				.id,
			"one:a",
			"several matches on an unclaimed extension go to the kind loaded first"
		);
		assert!(layered.magic_candidates(None).is_empty());
	}

	#[test]
	fn install_current_swaps_the_process_registry() {
		let before = FileTypeRegistry::current();
		assert!(before.type_by_extension(Path::new("a.zzzkind")).is_none());
		let installed = FileTypeRegistry::install_current(&[(
			"ext".to_string(),
			vec![kind("zz", ContentKind::Text, &["zzzkind"], &[])],
		)]);
		assert_eq!(
			FileTypeRegistry::current()
				.type_by_extension(Path::new("a.zzzkind"))
				.unwrap()
				.id,
			"ext:zz"
		);
		assert!(Arc::ptr_eq(&installed, &FileTypeRegistry::current()));
		FileTypeRegistry::install_current(&[]);
		assert!(FileTypeRegistry::current()
			.type_by_extension(Path::new("a.zzzkind"))
			.is_none());
	}
}
