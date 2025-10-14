//! File type registry - the main API for file type identification

use super::{FileType, FileTypeError, IdentificationMethod, IdentificationResult, Result};
use crate::domain::ContentKind;
use crate::filetype::magic::MagicBytePattern;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// Maximum bytes to read for magic byte identification
const MAX_MAGIC_BYTES: usize = 8192;

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
pub struct FileTypeRegistry {
	/// All registered file types by ID
	types: HashMap<String, FileType>,

	/// Extension to type IDs mapping
	extension_map: HashMap<String, Vec<String>>,

	/// MIME type to type ID mapping
	mime_map: HashMap<String, String>,
}

impl FileTypeRegistry {
	/// Create a new registry with built-in types
	pub fn new() -> Self {
		let mut registry = Self {
			types: HashMap::new(),
			extension_map: HashMap::new(),
			mime_map: HashMap::new(),
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
}
