//! # Text extraction for the content index
//!
//! Turns a file's bytes into the text the content index stores. Plain text,
//! Markdown, CSV, JSON and source code are read as text. PDF and DOCX are
//! parsed. Every extractor stops at [`MAX_TEXT_BYTES`] of text, so one huge
//! file costs the index a megabyte at most, and the hit says the file was cut.
//!
//! An extractor runs on a blocking thread. It never decides whether a file may
//! be opened: the caller has already refused a cloud placeholder, since
//! opening one downloads it.

use std::{
	fs::File,
	io::{self, BufReader, Read},
	path::Path,
};

use serde::{Deserialize, Serialize};
use specta::Type;

/// The text read per file. A file with more is indexed up to here.
pub const MAX_TEXT_BYTES: usize = 1_048_576;

/// The text read from a source file. Past this a file is usually generated
/// or vendored, and its start says what it is.
pub const CODE_TEXT_BYTES: usize = 256 * 1024;

/// The text read from data and logs: CSV, JSON and `.log` files. Their
/// rows and lines are mostly ids, numbers and timestamps that each cost the
/// index a posting of their own, and a search wants to find the file, which
/// its start does.
pub const DATA_TEXT_BYTES: usize = 64 * 1024;

/// Files read as text that hold nothing a person searches for, by name.
const SKIPPED_NAMES: &[&str] = &["package-lock.json", "npm-shrinkwrap.json", "pnpm-lock.yaml"];

/// The most text kept from one file of `kind` with `extension`.
///
/// Prose and documents are kept furthest, since they are what a person
/// searches. Code and data are cut sooner: on a developer's home folder they
/// were most of the files and, uncut, most of the index.
pub fn text_cap(kind: TextKind, extension: &str) -> usize {
	match kind {
		TextKind::Csv | TextKind::Json => DATA_TEXT_BYTES,
		TextKind::Text if extension.eq_ignore_ascii_case("log") => DATA_TEXT_BYTES,
		TextKind::Code => CODE_TEXT_BYTES,
		_ => MAX_TEXT_BYTES,
	}
}

/// Why a file is not read at all, judged by its name: lockfiles and
/// minified bundles are machine output whose every token is unique.
pub fn skip_by_name(name: &str) -> Option<&'static str> {
	let name = name.to_ascii_lowercase();
	if SKIPPED_NAMES.contains(&name.as_str()) {
		return Some("lockfile");
	}
	if name.contains(".min.") {
		return Some("minified");
	}
	None
}

/// PDF and DOCX files larger than this are not parsed. A PDF parse loads the
/// whole file, and a DOCX's document part is read through this bound, so it
/// is what keeps one document from taking the memory of a background pass.
pub const MAX_DOCUMENT_BYTES: u64 = 64 * 1024 * 1024;

/// The most a PDF's compressed streams may expand to before it is parsed.
/// The parser inflates every stream it reads in full, so a small file built
/// to expand would otherwise take the memory of the daemon.
pub const MAX_PDF_INFLATED_BYTES: u64 = 256 * 1024 * 1024;

/// Which extractor read a file. Search filters by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TextKind {
	Text,
	Markdown,
	Csv,
	Json,
	Code,
	Pdf,
	Docx,
	/// Words spoken in an audio or video file, from its transcript. No
	/// extension maps to it; the transcription job writes these rows.
	Transcript,
	/// What a video shot shows, from the Videos extension's `scene`
	/// sidecar. Written when the sidecar is, behind `video-shots`.
	Scene,
	/// What a photo shows, from the Photos extension's `caption` sidecar.
	/// Written when the sidecar is, behind `ai-cloud`.
	Caption,
}

/// File extensions the index reads, lowercase, and the extractor for each.
const EXTENSIONS: &[(&str, TextKind)] = &[
	("txt", TextKind::Text),
	("text", TextKind::Text),
	("log", TextKind::Text),
	("rst", TextKind::Text),
	("adoc", TextKind::Text),
	("org", TextKind::Text),
	("tex", TextKind::Text),
	("md", TextKind::Markdown),
	("markdown", TextKind::Markdown),
	("mdx", TextKind::Markdown),
	("csv", TextKind::Csv),
	("tsv", TextKind::Csv),
	("json", TextKind::Json),
	("jsonl", TextKind::Json),
	("ndjson", TextKind::Json),
	("geojson", TextKind::Json),
	("pdf", TextKind::Pdf),
	("docx", TextKind::Docx),
	("rs", TextKind::Code),
	("ts", TextKind::Code),
	("tsx", TextKind::Code),
	("js", TextKind::Code),
	("jsx", TextKind::Code),
	("mjs", TextKind::Code),
	("cjs", TextKind::Code),
	("py", TextKind::Code),
	("rb", TextKind::Code),
	("go", TextKind::Code),
	("java", TextKind::Code),
	("kt", TextKind::Code),
	("kts", TextKind::Code),
	("swift", TextKind::Code),
	("m", TextKind::Code),
	("mm", TextKind::Code),
	("c", TextKind::Code),
	("h", TextKind::Code),
	("cc", TextKind::Code),
	("cpp", TextKind::Code),
	("cxx", TextKind::Code),
	("hpp", TextKind::Code),
	("cs", TextKind::Code),
	("fs", TextKind::Code),
	("php", TextKind::Code),
	("sh", TextKind::Code),
	("bash", TextKind::Code),
	("zsh", TextKind::Code),
	("fish", TextKind::Code),
	("ps1", TextKind::Code),
	("lua", TextKind::Code),
	("sql", TextKind::Code),
	("r", TextKind::Code),
	("pl", TextKind::Code),
	("scala", TextKind::Code),
	("clj", TextKind::Code),
	("ex", TextKind::Code),
	("exs", TextKind::Code),
	("erl", TextKind::Code),
	("hs", TextKind::Code),
	("ml", TextKind::Code),
	("dart", TextKind::Code),
	("zig", TextKind::Code),
	("nim", TextKind::Code),
	("vue", TextKind::Code),
	("svelte", TextKind::Code),
	("html", TextKind::Code),
	("htm", TextKind::Code),
	("css", TextKind::Code),
	("scss", TextKind::Code),
	("less", TextKind::Code),
	("xml", TextKind::Code),
	("yaml", TextKind::Code),
	("yml", TextKind::Code),
	("toml", TextKind::Code),
	("ini", TextKind::Code),
	("cfg", TextKind::Code),
	("conf", TextKind::Code),
	("proto", TextKind::Code),
	("graphql", TextKind::Code),
	("tf", TextKind::Code),
	("gradle", TextKind::Code),
	("cmake", TextKind::Code),
];

impl TextKind {
	/// The extractor for a file extension, compared without case.
	pub fn for_extension(extension: &str) -> Option<Self> {
		let extension = extension.to_ascii_lowercase();
		EXTENSIONS
			.iter()
			.find(|(known, _)| *known == extension)
			.map(|(_, kind)| *kind)
	}

	/// Every extension the index reads, lowercase.
	pub fn extensions() -> Vec<&'static str> {
		EXTENSIONS.iter().map(|(extension, _)| *extension).collect()
	}

	/// The name the index stores and filters by.
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Text => "text",
			Self::Markdown => "markdown",
			Self::Csv => "csv",
			Self::Json => "json",
			Self::Code => "code",
			Self::Pdf => "pdf",
			Self::Docx => "docx",
			Self::Transcript => sd_store::TRANSCRIPT_KIND,
			Self::Scene => sd_store::SCENE_KIND,
			Self::Caption => sd_store::CAPTION_KIND,
		}
	}

	pub fn parse(name: &str) -> Option<Self> {
		[
			Self::Text,
			Self::Markdown,
			Self::Csv,
			Self::Json,
			Self::Code,
			Self::Pdf,
			Self::Docx,
			Self::Transcript,
			Self::Scene,
			Self::Caption,
		]
		.into_iter()
		.find(|kind| kind.as_str() == name)
	}
}

/// What reading a file produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Extraction {
	/// Text to index, and whether the file held more than the cap.
	Text { text: String, truncated: bool },
	/// The file was read and holds nothing to index, and why.
	Skipped(String),
}

/// Read the text of one file with the extractor for `kind`, keeping at most
/// `cap` bytes of it.
///
/// An error means the file could not be opened or read, which the caller
/// retries on a later pass. A file that opened but holds nothing usable is an
/// [`Extraction::Skipped`], which is recorded so it is not read again until
/// it changes.
pub fn extract(path: &Path, kind: TextKind, cap: usize) -> io::Result<Extraction> {
	if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
		return Ok(Extraction::Skipped("symbolic link".into()));
	}
	let extraction = match kind {
		TextKind::Pdf => read_pdf(path, cap)?,
		TextKind::Docx => read_docx(path, cap)?,
		_ => read_text(path, cap)?,
	};
	Ok(match extraction {
		Extraction::Text { text, .. } if text.trim().is_empty() => {
			Extraction::Skipped("no text".into())
		}
		other => other,
	})
}

/// Plain text and every format that is text on disk. A NUL byte in the first
/// `cap` bytes marks the file binary, which is the same test the live grep
/// applies.
fn read_text(path: &Path, cap: usize) -> io::Result<Extraction> {
	let mut bytes = Vec::new();
	File::open(path)?
		.take(cap as u64 + 1)
		.read_to_end(&mut bytes)?;
	let truncated = bytes.len() > cap;
	bytes.truncate(cap);
	if bytes.contains(&0) {
		return Ok(Extraction::Skipped("binary".into()));
	}
	if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
		bytes.drain(..3);
	}
	let mut text = match String::from_utf8(bytes) {
		Ok(text) => text,
		Err(error) => {
			let utf8 = error.utf8_error();
			let mut bytes = error.into_bytes();
			// Only a character the cap cut in half is dropped; anything
			// else invalid is kept as a replacement character.
			if truncated && utf8.error_len().is_none() {
				bytes.truncate(utf8.valid_up_to());
				String::from_utf8(bytes).unwrap_or_default()
			} else {
				String::from_utf8_lossy(&bytes).into_owned()
			}
		}
	};
	let cut = cap_text(&mut text, cap);
	Ok(Extraction::Text {
		text,
		truncated: truncated || cut,
	})
}

fn read_pdf(path: &Path, cap: usize) -> io::Result<Extraction> {
	if std::fs::metadata(path)?.len() > MAX_DOCUMENT_BYTES {
		return Ok(Extraction::Skipped("too large to parse".into()));
	}
	let bytes = std::fs::read(path)?;
	if let Err(reason) = pdf_preflight(&bytes, MAX_PDF_INFLATED_BYTES) {
		return Ok(Extraction::Skipped(format!("PDF {reason}")));
	}
	// The parser panics on some malformed files. One bad PDF is skipped
	// rather than taking the pass down with it.
	let parsed = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(&bytes));
	let mut text = match parsed {
		Ok(Ok(text)) => text,
		Ok(Err(error)) => return Ok(Extraction::Skipped(format!("unreadable PDF: {error}"))),
		Err(_) => return Ok(Extraction::Skipped("unreadable PDF".into())),
	};
	let truncated = cap_text(&mut text, cap);
	Ok(Extraction::Text { text, truncated })
}

/// Check that parsing a PDF stays within `limit` bytes of decoded streams.
///
/// The parser decodes every stream it reads in full, through whatever filter
/// chain the stream names, so a small file can be built to expand without
/// bound. Two checks run on the raw bytes before the parser sees the file.
/// Every `/Filter` in it must name no more than one filter, either Flate or
/// an image filter the parser leaves encoded; a chain, any other filter, or a
/// filter given by reference is refused rather than guessed at. Then the data
/// after every `stream` keyword is inflated through a bounded reader, and
/// the total must stay under `limit`. Data that is not zlib counts nothing.
/// Streams cannot sit inside object streams, so every stream and its
/// dictionary are in the raw bytes.
fn pdf_preflight(pdf: &[u8], limit: u64) -> Result<(), String> {
	let mut at = 0;
	while let Some(slash) = pdf[at..].iter().position(|&c| c == b'/') {
		let start = at + slash + 1;
		let (name, len) = pdf_name(&pdf[start..]);
		at = start + len;
		if name == b"Filter" {
			check_filter(&pdf[at..pdf.len().min(at + 512)])?;
		}
	}

	let mut total = 0u64;
	let mut at = 0;
	while let Some(found) = find(&pdf[at..], b"stream") {
		let start = at + found;
		at = start + b"stream".len();
		if start >= 3 && &pdf[start - 3..start] == b"end" {
			continue;
		}
		let mut data = at;
		if pdf.get(data) == Some(&b'\r') {
			data += 1;
		}
		if pdf.get(data) == Some(&b'\n') {
			data += 1;
		}
		let mut inflate = flate2::read::ZlibDecoder::new(&pdf[data..]).take(limit + 1 - total);
		total += io::copy(&mut inflate, &mut io::sink()).unwrap_or(0);
		if total > limit {
			return Err("expands too far to parse".into());
		}
	}
	Ok(())
}

/// Accept the value of one `/Filter` key if it names at most one filter
/// whose output the preflight bounds.
fn check_filter(value: &[u8]) -> Result<(), String> {
	let refuse = |what: &str| Err(format!("uses {what}"));
	let skip_space = |at: &mut usize| {
		while value.get(*at).is_some_and(|c| c.is_ascii_whitespace()) {
			*at += 1;
		}
	};
	let mut at = 0;
	skip_space(&mut at);
	let mut names = Vec::new();
	match value.get(at) {
		Some(b'/') => names.push(pdf_name(&value[at + 1..]).0),
		Some(b'[') => {
			at += 1;
			loop {
				skip_space(&mut at);
				match value.get(at) {
					Some(b']') => break,
					Some(b'/') => {
						let (name, len) = pdf_name(&value[at + 1..]);
						names.push(name);
						at += 1 + len;
					}
					Some(_) => return refuse("a stream filter given by reference"),
					None => return refuse("a malformed stream filter"),
				}
			}
		}
		_ => return refuse("a stream filter given by reference"),
	}
	match names.as_slice() {
		[] => Ok(()),
		[name] => match name.as_slice() {
			b"FlateDecode" | b"Fl" | b"DCTDecode" | b"DCT" | b"JPXDecode" | b"CCITTFaxDecode"
			| b"CCF" | b"JBIG2Decode" => Ok(()),
			_ => refuse("a stream filter the index does not bound"),
		},
		_ => refuse("a chain of stream filters"),
	}
}

/// The PDF name at the start of `bytes`, just after its slash, with `#hh`
/// escapes decoded the way the parser decodes them, and the bytes it took.
fn pdf_name(bytes: &[u8]) -> (Vec<u8>, usize) {
	let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
	let mut name = Vec::new();
	let mut at = 0;
	while let Some(&c) = bytes.get(at) {
		if c.is_ascii_whitespace() || c == 0 || b"/<>[]{}()%".contains(&c) {
			break;
		}
		if c == b'#' {
			if let (Some(high), Some(low)) = (
				bytes.get(at + 1).copied().and_then(hex),
				bytes.get(at + 2).copied().and_then(hex),
			) {
				name.push(high << 4 | low);
				at += 3;
				continue;
			}
		}
		name.push(c);
		at += 1;
	}
	(name, at)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
	haystack
		.windows(needle.len())
		.position(|window| window == needle)
}

/// The body of a Word document: the text runs of `word/document.xml`, one
/// line per paragraph. Read as a stream, so parsing stops at the cap.
fn read_docx(path: &Path, cap: usize) -> io::Result<Extraction> {
	use quick_xml::events::Event;

	let skipped = |what: &str, error: &dyn std::fmt::Display| {
		Ok(Extraction::Skipped(format!(
			"unreadable DOCX {what}: {error}"
		)))
	};
	let mut archive = match zip::ZipArchive::new(BufReader::new(File::open(path)?)) {
		Ok(archive) => archive,
		Err(zip::result::ZipError::Io(error)) => return Err(error),
		Err(error) => return skipped("archive", &error),
	};
	let part = match archive.by_name("word/document.xml") {
		Ok(part) => part,
		Err(error) => return skipped("body", &error),
	};
	let mut reader = quick_xml::Reader::from_reader(BufReader::new(part.take(MAX_DOCUMENT_BYTES)));
	let mut buf = Vec::new();
	let mut text = String::new();
	let mut in_run = false;
	let mut truncated = false;
	loop {
		match reader.read_event_into(&mut buf) {
			Ok(Event::Start(element)) if element.local_name().as_ref() == b"t" => in_run = true,
			Ok(Event::End(element)) => match element.local_name().as_ref() {
				b"t" => in_run = false,
				b"p" => text.push('\n'),
				_ => {}
			},
			Ok(Event::Empty(element)) => match element.local_name().as_ref() {
				b"tab" => text.push('\t'),
				b"br" | b"cr" => text.push('\n'),
				_ => {}
			},
			Ok(Event::Text(run)) if in_run => match run.decode() {
				Ok(run) => text.push_str(&run),
				Err(error) => return skipped("text", &error),
			},
			Ok(Event::GeneralRef(reference)) if in_run => match reference.resolve_char_ref() {
				Ok(Some(c)) => text.push(c),
				Ok(None) => match &*reference {
					b"amp" => text.push('&'),
					b"lt" => text.push('<'),
					b"gt" => text.push('>'),
					b"quot" => text.push('"'),
					b"apos" => text.push('\''),
					_ => {}
				},
				Err(error) => return skipped("text", &error),
			},
			Ok(Event::Eof) => break,
			Ok(_) => {}
			Err(quick_xml::Error::Io(error)) => {
				return Err(io::Error::new(error.kind(), error.to_string()))
			}
			Err(error) => return skipped("body", &error),
		}
		if text.len() > cap {
			truncated = true;
			break;
		}
		buf.clear();
	}
	cap_text(&mut text, cap);
	Ok(Extraction::Text { text, truncated })
}

/// Cut `text` to at most `cap` bytes on a character boundary. Returns whether
/// anything was cut.
fn cap_text(text: &mut String, cap: usize) -> bool {
	if text.len() <= cap {
		return false;
	}
	let mut end = cap;
	while !text.is_char_boundary(end) {
		end -= 1;
	}
	text.truncate(end);
	true
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::io::Write;

	fn write(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
		let path = dir.join(name);
		std::fs::write(&path, bytes).unwrap();
		path
	}

	fn text_of(path: &Path) -> (String, bool) {
		let name = path.file_name().unwrap().to_string_lossy();
		let extension = name.rsplit_once('.').unwrap().1;
		let kind = TextKind::for_extension(extension).expect("an indexed extension");
		match extract(path, kind, MAX_TEXT_BYTES).unwrap() {
			Extraction::Text { text, truncated } => (text, truncated),
			Extraction::Skipped(reason) => panic!("{name} skipped: {reason}"),
		}
	}

	#[test]
	fn every_extension_maps_to_one_kind() {
		let extensions = TextKind::extensions();
		let mut unique = extensions.clone();
		unique.sort();
		unique.dedup();
		assert_eq!(unique.len(), extensions.len());
		assert_eq!(TextKind::for_extension("MD"), Some(TextKind::Markdown));
		assert_eq!(TextKind::for_extension("jpg"), None);
		for kind in ["text", "markdown", "csv", "json", "code", "pdf", "docx"] {
			assert_eq!(TextKind::parse(kind).unwrap().as_str(), kind);
		}
	}

	#[test]
	fn text_formats_are_read_as_written() {
		let dir = tempfile::tempdir().unwrap();
		let cases: [(&str, &str); 5] = [
			("notes.txt", "plain words here"),
			("README.md", "# Title\n\nSome *markdown* with [a link](x)."),
			("table.csv", "name,city\nAda,London\n"),
			("data.json", "{\"key\": \"value with words\"}"),
			("main.rs", "fn main() {\n\tprintln!(\"hi\");\n}\n"),
		];
		for (name, body) in cases {
			let path = write(dir.path(), name, body.as_bytes());
			assert_eq!(text_of(&path), (body.to_string(), false), "{name}");
		}
	}

	#[test]
	fn a_byte_order_mark_is_dropped_and_bad_bytes_are_replaced() {
		let dir = tempfile::tempdir().unwrap();
		let path = write(dir.path(), "bom.txt", b"\xEF\xBB\xBFhello");
		assert_eq!(text_of(&path).0, "hello");
		let path = write(dir.path(), "latin1.txt", b"caf\xE9 au lait");
		assert_eq!(text_of(&path).0, "caf\u{FFFD} au lait");
	}

	#[test]
	fn binary_empty_and_linked_files_are_skipped() {
		let dir = tempfile::tempdir().unwrap();
		let binary = write(dir.path(), "blob.txt", b"text\0more");
		assert_eq!(
			extract(&binary, TextKind::Text, MAX_TEXT_BYTES).unwrap(),
			Extraction::Skipped("binary".into())
		);
		let empty = write(dir.path(), "empty.md", b"  \n");
		assert_eq!(
			extract(&empty, TextKind::Markdown, MAX_TEXT_BYTES).unwrap(),
			Extraction::Skipped("no text".into())
		);
		#[cfg(unix)]
		{
			let link = dir.path().join("link.txt");
			std::os::unix::fs::symlink(&empty, &link).unwrap();
			assert_eq!(
				extract(&link, TextKind::Text, MAX_TEXT_BYTES).unwrap(),
				Extraction::Skipped("symbolic link".into())
			);
		}
		assert!(extract(&dir.path().join("gone.txt"), TextKind::Text, 10).is_err());
	}

	#[test]
	fn text_stops_at_the_cap() {
		let dir = tempfile::tempdir().unwrap();
		let exact = write(dir.path(), "exact.txt", &vec![b'a'; MAX_TEXT_BYTES]);
		let (text, truncated) = text_of(&exact);
		assert_eq!((text.len(), truncated), (MAX_TEXT_BYTES, false));

		let over = write(dir.path(), "over.txt", &vec![b'a'; MAX_TEXT_BYTES + 1]);
		let (text, truncated) = text_of(&over);
		assert_eq!((text.len(), truncated), (MAX_TEXT_BYTES, true));

		// Two-byte characters with one leading byte put the cap inside a
		// character, which is dropped rather than replaced.
		let mut bytes = vec![b'x'];
		bytes.extend("é".repeat(MAX_TEXT_BYTES).bytes());
		let split = write(dir.path(), "split.txt", &bytes);
		let (text, truncated) = text_of(&split);
		assert!(truncated);
		assert_eq!(text.len(), MAX_TEXT_BYTES - 1);
		assert!(text.ends_with('é'));
	}

	/// A one-page PDF that shows `line` in Helvetica, with a correct xref.
	fn pdf(line: &str) -> Vec<u8> {
		let stream = format!("BT /F1 12 Tf 72 720 Td ({line}) Tj ET");
		let objects = [
			"<< /Type /Catalog /Pages 2 0 R >>".to_string(),
			"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
			"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".to_string(),
			format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()),
			"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_string(),
		];
		let mut out = b"%PDF-1.4\n".to_vec();
		let mut offsets = Vec::new();
		for (i, object) in objects.iter().enumerate() {
			offsets.push(out.len());
			out.extend(format!("{} 0 obj\n{object}\nendobj\n", i + 1).bytes());
		}
		let xref = out.len();
		out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
		for offset in offsets {
			out.extend(format!("{offset:010} 00000 n \n").bytes());
		}
		out.extend(
			format!(
				"trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
				objects.len() + 1
			)
			.bytes(),
		);
		out
	}

	fn flate(len: usize) -> Vec<u8> {
		use flate2::{write::ZlibEncoder, Compression};
		let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
		encoder.write_all(&vec![b' '; len]).unwrap();
		encoder.finish().unwrap()
	}

	fn stream_pdf(dictionary: &str, data: &[u8]) -> Vec<u8> {
		let mut pdf = format!("%PDF-1.4\n1 0 obj\n{dictionary}\nstream\n").into_bytes();
		pdf.extend(data);
		pdf.extend(b"\nendstream\nendobj\n");
		pdf
	}

	#[test]
	fn a_pdf_that_expands_too_far_is_not_parsed() {
		let bomb = stream_pdf("<< /Filter /FlateDecode >>", &flate(4 * 1024 * 1024));
		assert!(bomb.len() < 64 * 1024, "the file itself is small");
		assert!(pdf_preflight(&bomb, 1024).is_err());
		assert!(pdf_preflight(&bomb, 8 * 1024 * 1024).is_ok());
		let array = stream_pdf("<< /Length 9 /Filter [ /Fl ] >>", &flate(4096));
		assert!(pdf_preflight(&array, 1024).is_err());
		assert!(pdf_preflight(&array, 8192).is_ok());
		assert!(pdf_preflight(b"no streams here", 10).is_ok());
	}

	#[test]
	fn a_pdf_filter_the_check_cannot_bound_is_refused() {
		for dictionary in [
			"<< /Filter [/ASCII85Decode /FlateDecode] >>",
			"<< /Filter [/FlateDecode /FlateDecode] >>",
			"<< /Filter /LZWDecode >>",
			"<< /Filter 7 0 R >>",
			"<</Filter[/A85/Fl]/Length 4>>",
			"<< /F#69lter [/ASCII85Decode /FlateDecode] >>",
			"<< /Filter /LZW#44ecode >>",
		] {
			let pdf = stream_pdf(dictionary, b"data");
			assert!(
				pdf_preflight(&pdf, MAX_PDF_INFLATED_BYTES).is_err(),
				"{dictionary}"
			);
		}
		for dictionary in [
			"<< /Length 4 >>",
			"<< /Filter /DCTDecode >>",
			"<</Filter/DCTDecode/Length 4>>",
			"<< /Filter /Fl#61teDecode /FilterType 1 >>",
		] {
			let pdf = stream_pdf(dictionary, b"data");
			assert!(
				pdf_preflight(&pdf, MAX_PDF_INFLATED_BYTES).is_ok(),
				"{dictionary}"
			);
		}
	}

	#[test]
	fn a_pdf_yields_its_text_and_a_broken_one_is_skipped() {
		let dir = tempfile::tempdir().unwrap();
		let path = write(dir.path(), "paper.pdf", &pdf("Quarterly needle report"));
		let (text, truncated) = text_of(&path);
		assert!(text.contains("Quarterly needle report"), "{text:?}");
		assert!(!truncated);

		let capped = extract(&path, TextKind::Pdf, 9).unwrap();
		match capped {
			Extraction::Text { text, truncated } => {
				assert!(truncated);
				assert!(text.len() <= 9);
			}
			other => panic!("{other:?}"),
		}

		let broken = write(dir.path(), "broken.pdf", b"%PDF-1.4 not really");
		assert!(matches!(
			extract(&broken, TextKind::Pdf, MAX_TEXT_BYTES).unwrap(),
			Extraction::Skipped(_)
		));
	}

	fn docx(body: &str) -> Vec<u8> {
		let mut out = std::io::Cursor::new(Vec::new());
		let mut zip = zip::ZipWriter::new(&mut out);
		let options = zip::write::SimpleFileOptions::default();
		zip.start_file("[Content_Types].xml", options).unwrap();
		zip.write_all(b"<Types/>").unwrap();
		zip.start_file("word/document.xml", options).unwrap();
		write!(
			zip,
			"<?xml version=\"1.0\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}</w:body></w:document>"
		)
		.unwrap();
		zip.finish().unwrap();
		out.into_inner()
	}

	#[test]
	fn a_docx_yields_its_paragraphs() {
		let dir = tempfile::tempdir().unwrap();
		let path = write(
			dir.path(),
			"letter.docx",
			&docx(
				"<w:p><w:r><w:t>Dear </w:t></w:r><w:r><w:t xml:space=\"preserve\">Ada &amp; Co</w:t></w:r></w:p>\
				 <w:p><w:r><w:t>Second</w:t><w:tab/><w:t>line</w:t></w:r></w:p>\
				 <w:p><w:r><w:instrText>HYPERLINK</w:instrText></w:r></w:p>",
			),
		);
		assert_eq!(
			text_of(&path),
			("Dear Ada & Co\nSecond\tline\n\n".into(), false)
		);

		let long = format!(
			"<w:p><w:r><w:t>{}</w:t></w:r></w:p>",
			"word ".repeat(300_000)
		);
		let path = write(dir.path(), "long.docx", &docx(&long));
		let (text, truncated) = text_of(&path);
		assert!(truncated);
		assert_eq!(text.len(), MAX_TEXT_BYTES);

		let broken = write(dir.path(), "broken.docx", b"PK not a zip");
		assert!(matches!(
			extract(&broken, TextKind::Docx, MAX_TEXT_BYTES).unwrap(),
			Extraction::Skipped(_)
		));
	}
}
