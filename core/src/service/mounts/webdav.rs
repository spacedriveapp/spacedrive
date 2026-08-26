//! Read-only WebDAV over the ephemeral source arenas.
//!
//! One share exposes every registered source as a top-level collection, so a
//! single mount shows the whole library-visible world — attached volumes,
//! cloud roots, and detached drives browsing from their snapshots. Listings
//! and stat come from the arena; bytes come from the byte plane, which is
//! the only thing that knows where a source's content actually lives.

use super::provider::{self, remote_share_name, share_name, ByteError, MountTarget};
use crate::context::CoreContext;
use crate::ops::indexing::ephemeral::cache::SourceStatus;
use crate::ops::indexing::state::EntryKind;
use axum::{
	body::Body,
	extract::State,
	http::{header, HeaderMap, Method, StatusCode, Uri},
	response::{IntoResponse, Response},
	routing::any,
	Router,
};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::stream;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

const DAV_PREFIX: &str = "/dav";

pub fn router(context: Arc<CoreContext>) -> Router {
	Router::new()
		.route("/dav", any(handle))
		.route("/dav/", any(handle))
		.route("/dav/*rest", any(handle))
		.with_state(context)
}

async fn handle(
	State(context): State<Arc<CoreContext>>,
	method: Method,
	uri: Uri,
	headers: HeaderMap,
) -> Response {
	let rel = uri
		.path()
		.strip_prefix(DAV_PREFIX)
		.unwrap_or("")
		.trim_start_matches('/');
	let rel = percent_decode(rel);

	match method.as_str() {
		"OPTIONS" => options_response(),
		"PROPFIND" => propfind(&context, &rel, &headers).await,
		"GET" | "HEAD" => get(&context, &rel, &headers, method == Method::HEAD).await,
		// Read-only share: every mutating DAV verb is refused uniformly.
		"PUT" | "DELETE" | "MKCOL" | "MOVE" | "COPY" | "PROPPATCH" | "LOCK" | "UNLOCK" => {
			(StatusCode::FORBIDDEN, "read-only share").into_response()
		}
		_ => (StatusCode::METHOD_NOT_ALLOWED, "").into_response(),
	}
}

fn options_response() -> Response {
	Response::builder()
		.status(StatusCode::OK)
		.header("DAV", "1")
		.header("MS-Author-Via", "DAV")
		.header(header::ALLOW, "OPTIONS, GET, HEAD, PROPFIND")
		.body(Body::empty())
		.unwrap()
}

// ---------------------------------------------------------------- PROPFIND

async fn propfind(context: &Arc<CoreContext>, rel: &str, headers: &HeaderMap) -> Response {
	let depth_children = headers
		.get("Depth")
		.and_then(|v| v.to_str().ok())
		.map(|d| d.trim() != "0")
		.unwrap_or(true);

	let cache = context.ephemeral_cache();
	let sources = cache.sources();

	let mut responses = String::new();

	if rel.is_empty() {
		// The share root: every source is a top-level collection.
		push_collection(
			&mut responses,
			&format!("{DAV_PREFIX}/"),
			"Spacedrive",
			None,
		);
		if depth_children {
			for s in &sources {
				let name = share_name(&s.root, s.id);
				push_collection(
					&mut responses,
					&format!("{DAV_PREFIX}/{}/", href_encode(&name)),
					&name,
					None,
				);
			}
			for remote in super::peer::remote_shares().await {
				let name = remote_share_name(&remote);
				push_collection(
					&mut responses,
					&format!("{DAV_PREFIX}/{}/", href_encode(&name)),
					&name,
					None,
				);
			}
		}
		return multistatus(responses);
	}

	let Some(target) = provider::resolve_target(context, rel).await else {
		return (StatusCode::NOT_FOUND, "").into_response();
	};

	let (index, source_root, fallback_display, abs) = match &target {
		MountTarget::Local(source, abs) => {
			cache.ensure_restored(&source.root).await;
			(
				cache.resolve_index(abs),
				source.root.clone(),
				share_name(&source.root, source.id),
				abs.clone(),
			)
		}
		MountTarget::Remote(share, abs) => (
			share.index.clone(),
			share.info.root.clone(),
			remote_share_name(share),
			abs.clone(),
		),
	};
	let index = index.read().await;

	let self_href = format!("{DAV_PREFIX}/{}", href_encode(rel.trim_end_matches('/')));
	let is_root = abs == source_root;
	let self_meta = index.get_entry_ref(&abs);

	let self_is_dir = is_root
		|| self_meta
			.as_ref()
			.map(|m| m.kind == EntryKind::Directory)
			.unwrap_or(false);

	if !is_root && self_meta.is_none() {
		return (StatusCode::NOT_FOUND, "").into_response();
	}

	let display = abs
		.file_name()
		.map(|n| n.to_string_lossy().into_owned())
		.unwrap_or(fallback_display);

	if self_is_dir {
		push_collection(&mut responses, &format!("{self_href}/"), &display, None);
		if depth_children {
			if let Some(children) = index.list_directory(&abs) {
				for child in children {
					let Some(meta) = index.get_entry_ref(&child) else {
						continue;
					};
					let name = child
						.file_name()
						.map(|n| n.to_string_lossy().into_owned())
						.unwrap_or_default();
					let child_href = format!("{self_href}/{}", href_encode(&name));
					if meta.kind == EntryKind::Directory {
						push_collection(
							&mut responses,
							&format!("{child_href}/"),
							&name,
							meta.modified,
						);
					} else {
						push_file(&mut responses, &child_href, &name, meta.size, meta.modified);
					}
				}
			}
		}
	} else {
		let meta = self_meta.unwrap();
		push_file(
			&mut responses,
			&self_href,
			&display,
			meta.size,
			meta.modified,
		);
	}

	multistatus(responses)
}

fn multistatus(responses: String) -> Response {
	let body = format!(
		"<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<D:multistatus xmlns:D=\"DAV:\">{responses}</D:multistatus>"
	);
	Response::builder()
		.status(StatusCode::MULTI_STATUS)
		.header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
		.header("DAV", "1")
		.body(Body::from(body))
		.unwrap()
}

fn push_collection(
	out: &mut String,
	href: &str,
	name: &str,
	modified: Option<std::time::SystemTime>,
) {
	out.push_str(&format!(
		"<D:response><D:href>{}</D:href><D:propstat><D:prop>\
		 <D:displayname>{}</D:displayname>\
		 <D:resourcetype><D:collection/></D:resourcetype>{}\
		 </D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>",
		xml_escape(href),
		xml_escape(name),
		lastmod_prop(modified),
	));
}

fn push_file(
	out: &mut String,
	href: &str,
	name: &str,
	size: u64,
	modified: Option<std::time::SystemTime>,
) {
	out.push_str(&format!(
		"<D:response><D:href>{}</D:href><D:propstat><D:prop>\
		 <D:displayname>{}</D:displayname>\
		 <D:resourcetype/>\
		 <D:getcontentlength>{}</D:getcontentlength>\
		 <D:getcontenttype>{}</D:getcontenttype>{}\
		 </D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>",
		xml_escape(href),
		xml_escape(name),
		size,
		content_type_for(name),
		lastmod_prop(modified),
	));
}

fn lastmod_prop(modified: Option<std::time::SystemTime>) -> String {
	modified
		.map(|st| {
			let dt: DateTime<Utc> = st.into();
			format!(
				"<D:getlastmodified>{}</D:getlastmodified>",
				dt.format("%a, %d %b %Y %H:%M:%S GMT")
			)
		})
		.unwrap_or_default()
}

// --------------------------------------------------------------------- GET

async fn get(
	context: &Arc<CoreContext>,
	rel: &str,
	headers: &HeaderMap,
	head_only: bool,
) -> Response {
	let cache = context.ephemeral_cache();

	let Some(target) = provider::resolve_target(context, rel).await else {
		return (StatusCode::NOT_FOUND, "").into_response();
	};

	// Directories aren't fetchable; clients browse them via PROPFIND.
	{
		let (index, abs, root) = match &target {
			MountTarget::Local(source, abs) => {
				cache.ensure_restored(&source.root).await;
				(cache.resolve_index(abs), abs, source.root.clone())
			}
			MountTarget::Remote(share, abs) => (share.index.clone(), abs, share.info.root.clone()),
		};
		let index = index.read().await;
		match index.get_entry_ref(abs) {
			Some(meta) if meta.kind == EntryKind::Directory => {
				return (StatusCode::FORBIDDEN, "directory").into_response();
			}
			Some(_) => {}
			None if *abs == root => {
				return (StatusCode::FORBIDDEN, "directory").into_response();
			}
			None => return (StatusCode::NOT_FOUND, "").into_response(),
		}
	}

	let provider = match provider::provider_for(context, &target).await {
		Ok(provider) => provider,
		Err(err) => return byte_error_response(err),
	};
	let byte_target = target.byte_target();
	let len = match provider.stat(&byte_target).await {
		Ok(stat) => stat.size,
		Err(err) => return byte_error_response(err),
	};

	let range = match parse_range(headers, len) {
		RangeParse::None => None,
		RangeParse::Some(r) => Some(r),
		RangeParse::Unsatisfiable => {
			return Response::builder()
				.status(StatusCode::RANGE_NOT_SATISFIABLE)
				.header(header::CONTENT_RANGE, format!("bytes */{len}"))
				.body(Body::empty())
				.unwrap();
		}
	};
	let (start, end) = range.unwrap_or((0, len.saturating_sub(1)));
	let content_len = if len == 0 { 0 } else { end - start + 1 };

	let mut builder = Response::builder()
		.status(if range.is_some() {
			StatusCode::PARTIAL_CONTENT
		} else {
			StatusCode::OK
		})
		.header(header::ACCEPT_RANGES, "bytes")
		.header(header::CONTENT_LENGTH, content_len)
		.header(
			header::CONTENT_TYPE,
			content_type_for(&byte_target.path.to_string_lossy()),
		);
	if range.is_some() {
		builder = builder.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"));
	}
	if head_only || content_len == 0 {
		return builder.body(Body::empty()).unwrap();
	}

	// One streaming loop for every provider: ask for as much as the class
	// serves per call, advance by what came back, stop on a short read.
	let body_stream = stream::unfold(
		(provider, byte_target, start, end),
		|(provider, target, cursor, end)| async move {
			if cursor > end {
				return None;
			}
			let want = (end - cursor + 1).min(provider.max_read());
			let started = std::time::Instant::now();
			let trace_id = super::trace::file_id(&target.path);
			match provider.read_range(&target, cursor..cursor + want).await {
				Ok(bytes) if bytes.is_empty() => None,
				Ok(bytes) => {
					let advance = bytes.len() as u64;
					super::trace::record(
						super::trace::Frontend::WebDav,
						trace_id,
						cursor,
						advance,
						started,
					);
					Some((
						Ok::<Bytes, std::io::Error>(bytes),
						(provider, target, cursor + advance, end),
					))
				}
				Err(err) => Some((
					Err(std::io::Error::other(err.to_string())),
					(provider, target, end + 1, end),
				)),
			}
		},
	);

	builder.body(Body::from_stream(body_stream)).unwrap()
}

/// The one place a byte-plane failure becomes something a client reads.
fn byte_error_response(err: ByteError) -> Response {
	match err {
		ByteError::NotFound(msg) => (StatusCode::NOT_FOUND, msg).into_response(),
		ByteError::Unsupported(msg) => (StatusCode::NOT_IMPLEMENTED, msg).into_response(),
		other => (StatusCode::SERVICE_UNAVAILABLE, other.to_string()).into_response(),
	}
}

// ----------------------------------------------------------------- helpers

enum RangeParse {
	None,
	Some((u64, u64)),
	Unsatisfiable,
}

/// Single-range `bytes=` parser; multi-range requests fall back to the full
/// body, which is always a valid server response.
fn parse_range(headers: &HeaderMap, len: u64) -> RangeParse {
	let Some(value) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) else {
		return RangeParse::None;
	};
	let Some(spec) = value.trim().strip_prefix("bytes=") else {
		return RangeParse::None;
	};
	if spec.contains(',') || len == 0 {
		return RangeParse::None;
	}
	let mut parts = spec.splitn(2, '-');
	let start_s = parts.next().unwrap_or("").trim();
	let end_s = parts.next().unwrap_or("").trim();

	if start_s.is_empty() {
		// Suffix range: last N bytes.
		let Ok(suffix) = end_s.parse::<u64>() else {
			return RangeParse::None;
		};
		if suffix == 0 {
			return RangeParse::Unsatisfiable;
		}
		let start = len.saturating_sub(suffix);
		return RangeParse::Some((start, len - 1));
	}

	let Ok(start) = start_s.parse::<u64>() else {
		return RangeParse::None;
	};
	if start >= len {
		return RangeParse::Unsatisfiable;
	}
	let end = if end_s.is_empty() {
		len - 1
	} else {
		match end_s.parse::<u64>() {
			Ok(e) => e.min(len - 1),
			Err(_) => return RangeParse::None,
		}
	};
	if end < start {
		return RangeParse::Unsatisfiable;
	}
	RangeParse::Some((start, end))
}

/// Extension-derived media type, so Finder previews, QuickLook, and video
/// players treat streamed files as what they are.
fn content_type_for(name: &str) -> &'static str {
	let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
	match ext.as_str() {
		"mp4" | "m4v" => "video/mp4",
		"mov" => "video/quicktime",
		"mkv" => "video/x-matroska",
		"webm" => "video/webm",
		"avi" => "video/x-msvideo",
		"mp3" => "audio/mpeg",
		"m4a" => "audio/mp4",
		"wav" => "audio/wav",
		"flac" => "audio/flac",
		"jpg" | "jpeg" => "image/jpeg",
		"png" => "image/png",
		"gif" => "image/gif",
		"webp" => "image/webp",
		"heic" => "image/heic",
		"svg" => "image/svg+xml",
		"pdf" => "application/pdf",
		"txt" | "log" => "text/plain",
		"md" => "text/markdown",
		"html" | "htm" => "text/html",
		"css" => "text/css",
		"json" => "application/json",
		"xml" => "application/xml",
		"zip" => "application/zip",
		_ => "application/octet-stream",
	}
}

fn xml_escape(s: &str) -> String {
	s.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
}

/// Percent-encode a path for use inside an href, keeping '/' separators.
fn href_encode(s: &str) -> String {
	let mut out = String::with_capacity(s.len());
	for byte in s.bytes() {
		match byte {
			b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
				out.push(byte as char)
			}
			_ => out.push_str(&format!("%{byte:02X}")),
		}
	}
	out
}

fn percent_decode(s: &str) -> String {
	let bytes = s.as_bytes();
	let mut out = Vec::with_capacity(bytes.len());
	let mut i = 0;
	while i < bytes.len() {
		if bytes[i] == b'%' && i + 2 < bytes.len() {
			if let Ok(v) =
				u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
			{
				out.push(v);
				i += 3;
				continue;
			}
		}
		out.push(bytes[i]);
		i += 1;
	}
	String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn range_parsing() {
		let mut headers = HeaderMap::new();
		headers.insert(header::RANGE, "bytes=0-99".parse().unwrap());
		assert!(matches!(
			parse_range(&headers, 1000),
			RangeParse::Some((0, 99))
		));

		headers.insert(header::RANGE, "bytes=500-".parse().unwrap());
		assert!(matches!(
			parse_range(&headers, 1000),
			RangeParse::Some((500, 999))
		));

		headers.insert(header::RANGE, "bytes=-100".parse().unwrap());
		assert!(matches!(
			parse_range(&headers, 1000),
			RangeParse::Some((900, 999))
		));

		headers.insert(header::RANGE, "bytes=2000-".parse().unwrap());
		assert!(matches!(
			parse_range(&headers, 1000),
			RangeParse::Unsatisfiable
		));
	}

	#[test]
	fn traversal_refused() {
		let sources = vec![SourceStatus {
			id: Uuid::now_v7(),
			root: PathBuf::from("/tmp/demo"),
			volume_uuid: None,
			attached: true,
			restored: false,
			last_seen_secs: 0,
			entry_count: None,
			total_bytes: None,
			directory: None,
			thumbs_path: None,
		}];
		let name = share_name(Path::new("/tmp/demo"), sources[0].id);
		assert!(
			provider::resolve_for_test(&sources, &format!("{name}/../../etc/passwd")).is_none()
		);
		assert!(provider::resolve_for_test(&sources, &format!("{name}/ok/file.txt")).is_some());
	}

	#[test]
	fn share_names_for_uri_roots() {
		let id = Uuid::now_v7();
		let n = share_name(Path::new("s3://my-bucket"), id);
		assert!(n.starts_with("my-bucket-"));
		let n = share_name(Path::new("/"), id);
		assert!(n.starts_with("root-"));
	}

	#[test]
	fn percent_roundtrip() {
		let original = "Time Machine/2026 α.txt";
		assert_eq!(percent_decode(&href_encode(original)), original);
	}
}
