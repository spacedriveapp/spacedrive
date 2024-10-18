use crate::util::InfallibleResponse;

use std::{fs::Metadata, time::UNIX_EPOCH};

use axum::{
	body::Body,
	http::{header, request, HeaderValue, Method, Response, StatusCode},
};
use http_range::HttpRange;
use tokio::{
	fs::File,
	io::{self, AsyncReadExt, AsyncSeekExt, SeekFrom},
};
use tokio_util::io::ReaderStream;
use tracing::error;

use super::utils::*;

// default capacity 64KiB
const DEFAULT_CAPACITY: usize = 65536;

/// Serve a Tokio file as a HTTP response.
///
/// This function takes care of:
///  - 304 Not Modified using ETag's
///  - Range requests for partial content
///
/// BE AWARE this function does not do any path traversal protection so that's up to the caller!
pub(crate) async fn serve_file(
	mut file: File,
	metadata: io::Result<Metadata>,
	req: request::Parts,
	mut resp: InfallibleResponse,
) -> Result<Response<Body>, Response<Body>> {
	if let Ok(metadata) = metadata {
		// We only accept range queries if `files.metadata() == Ok(_)`
		// https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Accept-Ranges
		resp = resp
			.header("Accept-Ranges", HeaderValue::from_static("bytes"))
			.header(
				"Content-Length",
				HeaderValue::from_str(&metadata.len().to_string())
					.expect("number won't fail conversion"),
			);

		// Empty files
		if metadata.len() == 0 {
			return Ok(resp
				.status(StatusCode::OK)
				.header("Content-Length", HeaderValue::from_static("0"))
				.body(Body::from("")));
		}

		// ETag
		let mut status_code = StatusCode::PARTIAL_CONTENT;
		if let Ok(time) = metadata.modified() {
			let etag_header =
				format!(
				r#""{}""#,
				// The ETag's can be any value so we just use the modified time to make it easy.
				time.duration_since(UNIX_EPOCH)
					.expect("are you a time traveler? cause that's the only explanation for this error")
					.as_millis()
			);

			// https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/ETag
			if let Ok(etag_header) = HeaderValue::from_str(&etag_header) {
				resp = resp.header("etag", etag_header);
			} else {
				error!("Failed to convert ETag into header value!");
			}

			// Used for normal requests
			if let Some(etag) = req.headers.get("If-None-Match") {
				if etag.as_bytes() == etag_header.as_bytes() {
					return Ok(resp.status(StatusCode::NOT_MODIFIED).body(Body::from("")));
				}
			}

			// Used checking if the resource has been modified since starting the download
			if let Some(if_range) = req.headers.get("If-Range") {
				// https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/If-Range
				if if_range.as_bytes() != etag_header.as_bytes() {
					status_code = StatusCode::OK
				}
			}
		};

		// https://developer.mozilla.org/en-US/docs/Web/HTTP/Range_requests
		if req.method == Method::GET {
			if let Some(range) = req.headers.get("range") {
				// TODO: Error handling
				let ranges = HttpRange::parse(range.to_str().map_err(bad_request)?, metadata.len())
					.map_err(bad_request)?;

				// TODO: Multipart requests are not support, yet
				if ranges.len() != 1 {
					return Ok(resp
						.header(
							header::CONTENT_RANGE,
							HeaderValue::from_str(&format!("bytes */{}", metadata.len()))
								.map_err(internal_server_error)?,
						)
						.status(StatusCode::RANGE_NOT_SATISFIABLE)
						.body(Body::from("")));
				}
				let range = ranges.first().expect("checked above");

				if (range.start + range.length) > metadata.len() {
					return Ok(resp
						.header(
							header::CONTENT_RANGE,
							HeaderValue::from_str(&format!("bytes */{}", metadata.len()))
								.map_err(internal_server_error)?,
						)
						.status(StatusCode::RANGE_NOT_SATISFIABLE)
						.body(Body::from("")));
				}

				file.seek(SeekFrom::Start(range.start))
					.await
					.map_err(internal_server_error)?;

				return Ok(resp
					.status(status_code)
					.header(
						"Content-Range",
						HeaderValue::from_str(&format!(
							"bytes {}-{}/{}",
							range.start,
							range.start + range.length - 1,
							metadata.len()
						))
						.map_err(internal_server_error)?,
					)
					.header(
						"Content-Length",
						HeaderValue::from_str(&range.length.to_string())
							.map_err(internal_server_error)?,
					)
					.body(Body::from_stream(ReaderStream::with_capacity(
						file.take(range.length),
						DEFAULT_CAPACITY,
					))));
			}
		}
	}

	Ok(resp.body(Body::from_stream(ReaderStream::new(file))))
}
