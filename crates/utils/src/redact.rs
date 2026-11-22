//! # Credential redaction for logs and client-facing text
//!
//! A URL or mount source can carry a username and password in its userinfo,
//! as in `smb://user:secret@host/share` or the `//user@host/share` form a
//! mount table reports. Anything that reaches a log file, a log stream, an
//! event or an error a client sees passes through
//! [`redact_url_credentials`] so neither half of the userinfo is written
//! out. The host, port and path stay, since those are what make a log line
//! useful.
//!
//! ## Example
//! ```rust
//! use sd_utils::redact::redact_url_credentials;
//!
//! let line = redact_url_credentials("mounting smb://user:secret@host/share");
//! assert_eq!(line, "mounting smb://***@host/share");
//! ```

use std::{borrow::Cow, io};

/// What replaces the userinfo of a URL.
pub const REDACTED: &str = "***";

/// Replaces the userinfo of every URL or network path in `text` with
/// [`REDACTED`].
///
/// The scan works on free text rather than parsing one URL, so it also
/// covers a URL inside an error message, a JSON payload or a log line.
/// Every `//` opens an authority that runs to the next `/`, `?`, `#`,
/// double quote, backslash, angle bracket, backtick or whitespace, and
/// everything before the last `@` in it is userinfo. Apostrophes are valid
/// in userinfo, so they do not end it. Taking the last `@` keeps an
/// unencoded `@` in a password from leaking the rest of it. Text with nothing to redact is
/// returned borrowed, so the common case does not allocate.
pub fn redact_url_credentials(text: &str) -> Cow<'_, str> {
	if !text.contains('@') || !text.contains("//") {
		return Cow::Borrowed(text);
	}

	let mut redacted = String::new();
	let mut copied = 0;
	let mut cursor = 0;
	while let Some(found) = text[cursor..].find("//") {
		let start = cursor + found + 2;
		let end = text[start..]
			.find(is_authority_end)
			.map_or(text.len(), |offset| start + offset);
		if let Some(at) = text[start..end].rfind('@').filter(|&at| at > 0) {
			redacted.push_str(&text[copied..start]);
			redacted.push_str(REDACTED);
			copied = start + at;
		}
		cursor = end;
	}

	if copied == 0 {
		return Cow::Borrowed(text);
	}
	redacted.push_str(&text[copied..]);
	Cow::Owned(redacted)
}

fn is_authority_end(c: char) -> bool {
	c.is_whitespace() || matches!(c, '/' | '?' | '#' | '"' | '\\' | '<' | '>' | '`')
}

/// An `io::Write` that redacts URL credentials from what passes through it.
///
/// Log formatters such as `tracing_subscriber::fmt` render a whole event
/// before writing it in one call, so each write holds complete lines and a
/// URL is never split across two writes. Bytes that are not UTF-8 pass
/// through untouched.
pub struct RedactingWriter<W> {
	inner: W,
}

impl<W> RedactingWriter<W> {
	pub fn new(inner: W) -> Self {
		Self { inner }
	}
}

impl<W: io::Write> io::Write for RedactingWriter<W> {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		match std::str::from_utf8(buf) {
			Ok(text) => self
				.inner
				.write_all(redact_url_credentials(text).as_bytes())?,
			Err(_) => self.inner.write_all(buf)?,
		}
		Ok(buf.len())
	}

	fn flush(&mut self) -> io::Result<()> {
		self.inner.flush()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::io::Write;

	#[test]
	fn redacts_user_and_password() {
		assert_eq!(
			redact_url_credentials("smb://alice:hunter2@nas.example:445/media"),
			"smb://***@nas.example:445/media"
		);
	}

	#[test]
	fn redacts_user_only() {
		assert_eq!(
			redact_url_credentials("ftp://alice@files.example/pub"),
			"ftp://***@files.example/pub"
		);
	}

	#[test]
	fn redacts_mount_table_source() {
		assert_eq!(
			redact_url_credentials("//alice@nas.example/media"),
			"//***@nas.example/media"
		);
		assert_eq!(
			redact_url_credentials("//alice:hunter2@nas.example/media"),
			"//***@nas.example/media"
		);
	}

	#[test]
	fn password_with_at_sign_is_fully_hidden() {
		assert_eq!(
			redact_url_credentials("smb://alice:p@ss@nas.example/media"),
			"smb://***@nas.example/media"
		);
	}

	#[test]
	fn password_with_apostrophe_is_fully_hidden() {
		assert_eq!(
			redact_url_credentials("smb://spacedrive:pa'ssword@127.0.0.1:1/spacedrive"),
			"smb://***@127.0.0.1:1/spacedrive"
		);
		assert_eq!(
			redact_url_credentials("url='smb://u:it's@host/share'"),
			"url='smb://***@host/share'"
		);
	}

	#[test]
	fn redacts_every_url_in_free_text() {
		assert_eq!(
			redact_url_credentials(
				"copy from smb://a:b@one.example/x to webdav://c:d@two.example/y failed"
			),
			"copy from smb://***@one.example/x to webdav://***@two.example/y failed"
		);
	}

	#[test]
	fn redacts_inside_json_and_debug_output() {
		assert_eq!(
			redact_url_credentials(
				r#"{"smb_url":"smb://spacedrive:abc123@127.0.0.1:50123/spacedrive"}"#
			),
			r#"{"smb_url":"smb://***@127.0.0.1:50123/spacedrive"}"#
		);
		assert_eq!(
			redact_url_credentials(r#"String("smb://u:p@h/s")"#),
			r#"String("smb://***@h/s")"#
		);
	}

	#[test]
	fn redacts_ipv6_host_and_url_without_path() {
		assert_eq!(
			redact_url_credentials("smb://u:p@[::1]:445"),
			"smb://***@[::1]:445"
		);
		assert_eq!(
			redact_url_credentials("at smb://u:p@host."),
			"at smb://***@host."
		);
	}

	#[test]
	fn leaves_text_without_credentials_untouched() {
		for text in [
			"smb://127.0.0.1:50123/spacedrive",
			"mail alice@example.com about it",
			"http://example.com/users/@alice",
			"//nas.example/media",
			"//@host/share",
			"",
		] {
			assert!(
				matches!(redact_url_credentials(text), Cow::Borrowed(t) if t == text),
				"changed {text:?}"
			);
		}
	}

	#[test]
	fn keeps_multibyte_text_around_urls() {
		assert_eq!(
			redact_url_credentials("ファイル smb://u:p@ホスト/共有 ✓"),
			"ファイル smb://***@ホスト/共有 ✓"
		);
	}

	#[test]
	fn writer_redacts_what_it_writes() {
		let mut out = Vec::new();
		let mut writer = RedactingWriter::new(&mut out);
		writer
			.write_all(b"INFO share at smb://spacedrive:abc123@127.0.0.1:1/spacedrive\n")
			.unwrap();
		writer.write_all(&[0xff, 0xfe]).unwrap();
		assert_eq!(
			out,
			b"INFO share at smb://***@127.0.0.1:1/spacedrive\n\xff\xfe".to_vec()
		);
	}
}
