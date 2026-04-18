//! One-shot loopback HTTP server for OAuth redirect capture (RFC 8252).
//!
//! The server handles exactly one request: the browser redirect that follows
//! user consent. The code is hand-rolled on `tokio::net::TcpListener` rather
//! than pulled from `hyper` because the request surface is trivial (a single
//! GET with query params) and because adding `hyper` as a direct sd-core dep
//! would pull a large dependency closure for 60 lines of parsing.
//!
//! Security notes:
//! - The CSRF `state` parameter is compared with [`subtle::ConstantTimeEq`] to
//!   foreclose timing attacks, even though the attack surface is narrow on
//!   loopback.
//! - The response contains no secrets — the HTML body is a static banner.
//! - Only the first callback is honored; subsequent connections would be
//!   ignored because the listener is dropped after `accept()`.

use super::error::OauthError;
use std::time::Duration;
use subtle::ConstantTimeEq;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::watch;

/// Result of a successful loopback capture.
#[derive(Debug, Clone)]
pub struct CallbackResult {
	/// Authorization code returned by the provider's `authorize` endpoint.
	pub code: String,
}

/// Static HTML served to the user's browser after capture.
///
/// Kept minimal so the response fits in a single write and does not require
/// the user to download additional assets across the loopback interface.
const SUCCESS_BODY: &str = "<!doctype html>\n\
<html><head><meta charset=\"utf-8\"><title>Spacedrive</title></head>\n\
<body style=\"font-family:system-ui;text-align:center;padding:4rem;\">\n\
<h1>Connected</h1><p>You can close this tab and return to Spacedrive.</p>\n\
</body></html>";

/// Run a one-shot HTTP server on `127.0.0.1:{port}` that accepts exactly one
/// OAuth redirect callback, validates the `state` parameter, and returns the
/// authorization `code`.
///
/// Times out after `timeout`. If `cancel` transitions to `true`, the function
/// resolves with `Err(OauthError::Timeout)` so callers can reuse the same
/// error path as a genuine timeout.
pub async fn run_loopback_callback_server(
	listener: TcpListener,
	expected_state: String,
	timeout: Duration,
	mut cancel: watch::Receiver<bool>,
) -> Result<CallbackResult, OauthError> {
	let accept = async {
		loop {
			let (mut stream, _) = listener
				.accept()
				.await
				.map_err(|e| OauthError::Loopback(e.to_string()))?;

			// OAuth redirect is a single GET; an 8 KiB buffer is larger than any
			// real redirect URL (browsers cap around 2 KiB) but still bounded so
			// a misbehaving client cannot exhaust memory.
			let mut buf = vec![0u8; 8192];
			let mut total = 0usize;
			loop {
				let n = stream
					.read(&mut buf[total..])
					.await
					.map_err(|e| OauthError::Loopback(e.to_string()))?;
				if n == 0 {
					break;
				}
				total += n;
				// A fully-received GET ends with \r\n\r\n; stop reading once we have headers.
				if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") || total == buf.len() {
					break;
				}
			}

			let request = String::from_utf8_lossy(&buf[..total]);
			let Some(request_line) = request.lines().next() else {
				send_error_response(&mut stream, "Empty request").await;
				continue;
			};

			let Some(path) = request_line.split_whitespace().nth(1) else {
				send_error_response(&mut stream, "Malformed request").await;
				continue;
			};

			let params = parse_query(path);

			if let Some(err) = params.get("error") {
				let detail = params
					.get("error_description")
					.map(|s| s.as_str())
					.unwrap_or(err);
				send_success_response(&mut stream).await;
				if err == "access_denied" {
					return Err(OauthError::UserDenied);
				}
				return Err(OauthError::TokenExchange(detail.to_string()));
			}

			let Some(code) = params.get("code").cloned() else {
				send_error_response(&mut stream, "Missing code").await;
				continue;
			};

			let Some(state) = params.get("state") else {
				send_error_response(&mut stream, "Missing state").await;
				continue;
			};

			if state
				.as_bytes()
				.ct_eq(expected_state.as_bytes())
				.unwrap_u8() != 1
			{
				send_error_response(&mut stream, "State mismatch").await;
				return Err(OauthError::StateMismatch);
			}

			send_success_response(&mut stream).await;
			return Ok(CallbackResult { code });
		}
	};

	tokio::select! {
		res = accept => res,
		_ = tokio::time::sleep(timeout) => Err(OauthError::Timeout),
		_ = cancel.wait_for(|v| *v) => Err(OauthError::Timeout),
	}
}

/// Best-effort write of the success HTML; callers cannot meaningfully recover
/// from a write failure at this point, so we swallow the error.
async fn send_success_response(stream: &mut tokio::net::TcpStream) {
	let response = format!(
		"HTTP/1.1 200 OK\r\n\
		 Content-Type: text/html; charset=utf-8\r\n\
		 Content-Length: {}\r\n\
		 Connection: close\r\n\r\n{}",
		SUCCESS_BODY.len(),
		SUCCESS_BODY
	);
	let _ = stream.write_all(response.as_bytes()).await;
	let _ = stream.shutdown().await;
}

/// Emit a 400 with a textual reason; used for malformed callbacks that should
/// not end the listener loop (we keep waiting for a valid callback).
async fn send_error_response(stream: &mut tokio::net::TcpStream, reason: &str) {
	let body = format!("OAuth callback error: {reason}\n");
	let response = format!(
		"HTTP/1.1 400 Bad Request\r\n\
		 Content-Type: text/plain; charset=utf-8\r\n\
		 Content-Length: {}\r\n\
		 Connection: close\r\n\r\n{}",
		body.len(),
		body
	);
	let _ = stream.write_all(response.as_bytes()).await;
	let _ = stream.shutdown().await;
}

/// Parse the query string portion of a request target into a map.
///
/// The decoder is deliberately small: OAuth providers emit well-formed URIs,
/// and bringing a full URL parser in for three query params is not warranted.
fn parse_query(target: &str) -> std::collections::HashMap<String, String> {
	let mut out = std::collections::HashMap::new();
	let Some(qs_idx) = target.find('?') else {
		return out;
	};
	let qs = &target[qs_idx + 1..];
	for pair in qs.split('&') {
		if pair.is_empty() {
			continue;
		}
		let (k, v) = match pair.split_once('=') {
			Some((k, v)) => (k, v),
			None => (pair, ""),
		};
		out.insert(url_decode(k), url_decode(v));
	}
	out
}

/// Percent-decode a small string, replacing `+` with space per
/// `application/x-www-form-urlencoded`. Invalid escapes are left literal so we
/// never panic on malformed input.
fn url_decode(input: &str) -> String {
	let mut out = String::with_capacity(input.len());
	let bytes = input.as_bytes();
	let mut i = 0;
	while i < bytes.len() {
		match bytes[i] {
			b'+' => {
				out.push(' ');
				i += 1;
			}
			b'%' if i + 2 < bytes.len() => {
				let hi = hex_digit(bytes[i + 1]);
				let lo = hex_digit(bytes[i + 2]);
				if let (Some(hi), Some(lo)) = (hi, lo) {
					out.push((hi * 16 + lo) as char);
					i += 3;
				} else {
					out.push('%');
					i += 1;
				}
			}
			b => {
				out.push(b as char);
				i += 1;
			}
		}
	}
	out
}

fn hex_digit(b: u8) -> Option<u8> {
	match b {
		b'0'..=b'9' => Some(b - b'0'),
		b'a'..=b'f' => Some(b - b'a' + 10),
		b'A'..=b'F' => Some(b - b'A' + 10),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use tokio::io::AsyncWriteExt;
	use tokio::net::TcpStream;

	async fn bind_localhost() -> (TcpListener, u16) {
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let port = listener.local_addr().unwrap().port();
		(listener, port)
	}

	async fn send_request(port: u16, target: &str) {
		let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
		let req = format!("GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n");
		stream.write_all(req.as_bytes()).await.unwrap();
		// Read and discard response.
		let mut buf = vec![0u8; 1024];
		let _ = stream.read(&mut buf).await;
	}

	#[tokio::test]
	async fn test_loopback_accepts_callback() {
		let (listener, port) = bind_localhost().await;
		let (_tx, rx) = watch::channel(false);

		let server = tokio::spawn(run_loopback_callback_server(
			listener,
			"mystate".to_string(),
			Duration::from_secs(5),
			rx,
		));

		send_request(port, "/?code=abc123&state=mystate").await;

		let result = server.await.unwrap().unwrap();
		assert_eq!(result.code, "abc123");
	}

	#[tokio::test]
	async fn test_loopback_rejects_state_mismatch() {
		let (listener, port) = bind_localhost().await;
		let (_tx, rx) = watch::channel(false);

		let server = tokio::spawn(run_loopback_callback_server(
			listener,
			"expected".to_string(),
			Duration::from_secs(5),
			rx,
		));

		send_request(port, "/?code=abc&state=attacker").await;

		let err = server.await.unwrap().unwrap_err();
		assert!(matches!(err, OauthError::StateMismatch));
	}

	#[tokio::test]
	async fn test_loopback_user_denied() {
		let (listener, port) = bind_localhost().await;
		let (_tx, rx) = watch::channel(false);

		let server = tokio::spawn(run_loopback_callback_server(
			listener,
			"s".to_string(),
			Duration::from_secs(5),
			rx,
		));

		send_request(port, "/?error=access_denied&error_description=User+denied").await;

		let err = server.await.unwrap().unwrap_err();
		assert!(matches!(err, OauthError::UserDenied));
	}

	#[tokio::test]
	async fn test_loopback_timeout() {
		let (listener, _port) = bind_localhost().await;
		let (_tx, rx) = watch::channel(false);

		let err =
			run_loopback_callback_server(listener, "s".to_string(), Duration::from_millis(100), rx)
				.await
				.unwrap_err();

		assert!(matches!(err, OauthError::Timeout));
	}

	#[tokio::test]
	async fn test_loopback_cancelled() {
		let (listener, _port) = bind_localhost().await;
		let (tx, rx) = watch::channel(false);

		let server = tokio::spawn(run_loopback_callback_server(
			listener,
			"s".to_string(),
			Duration::from_secs(5),
			rx,
		));

		tokio::time::sleep(Duration::from_millis(20)).await;
		tx.send(true).unwrap();

		let err = server.await.unwrap().unwrap_err();
		assert!(matches!(err, OauthError::Timeout));
	}

	#[test]
	fn test_parse_query_basic() {
		let m = parse_query("/oauth/callback?code=abc&state=xyz");
		assert_eq!(m.get("code").map(String::as_str), Some("abc"));
		assert_eq!(m.get("state").map(String::as_str), Some("xyz"));
	}

	#[test]
	fn test_parse_query_url_encoded() {
		let m = parse_query("/?error=access_denied&error_description=User+denied+access");
		assert_eq!(
			m.get("error_description").map(String::as_str),
			Some("User denied access")
		);
	}
}
