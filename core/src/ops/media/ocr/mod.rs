//! OCR (Optical Character Recognition) system
//!
//! Extracts text from images and PDFs using tesseract-rs.
//! Text is stored in content_identity.text_content field.

pub mod action;
pub mod job;
pub mod processor;

pub use action::{ExtractTextAction, ExtractTextInput, ExtractTextOutput};
pub use job::{OcrJob, OcrJobConfig};
pub use processor::OcrProcessor;

use anyhow::Result;
use std::path::Path;

/// Extract text from an image or PDF using tesseract
pub async fn extract_text_from_file(source_path: &Path, languages: &[String]) -> Result<String> {
	use tokio::task::spawn_blocking;

	let source = source_path.to_path_buf();
	let langs = languages.to_vec();

	// Run OCR in blocking task (CPU intensive)
	spawn_blocking(move || {
		// TODO: Integrate tesseract-rs
		// Add to Cargo.toml:
		// tesseract = { version = "0.14", optional = true }
		//
		// Then implement:
		// #[cfg(feature = "tesseract")]
		// {
		//     let tesseract = tesseract::Tesseract::new(None, Some(&langs.join("+")))?;
		//     tesseract.set_image(&source.to_string_lossy())?;
		//     let text = tesseract.get_text()?;
		//     Ok(text)
		// }
		// #[cfg(not(feature = "tesseract"))]
		// {
		//     Err(anyhow::anyhow!("Tesseract feature not enabled"))
		// }

		// Placeholder implementation
		Ok(format!(
			"[OCR placeholder - tesseract integration needed for {}]",
			source.file_name().unwrap().to_string_lossy()
		))
	})
	.await?
}

/// Check if a file type supports OCR based on content kind
pub fn is_ocr_supported(mime_type: &str) -> bool {
	use crate::domain::ContentKind;
	use crate::filetype::FileTypeRegistry;

	// Create registry instance (consider caching this globally)
	let registry = FileTypeRegistry::new();

	// Get file type by MIME
	if let Some(file_type) = registry.get_by_mime(mime_type) {
		// OCR supported for images and documents
		matches!(
			file_type.category,
			ContentKind::Image | ContentKind::Document
		)
	} else {
		// Fallback to direct MIME check for unknown types
		mime_type.starts_with("image/") || mime_type == "application/pdf"
	}
}
