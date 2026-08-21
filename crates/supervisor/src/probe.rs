use std::time::Duration;
use tokio::{
	io::{AsyncReadExt, AsyncWriteExt},
	net::TcpStream,
	time::timeout,
};

#[derive(Debug, Clone)]
pub struct ProbeResult {
	pub ok: bool,
	pub detail: String,
}

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_REDIRECTS: usize = 5;

/// Probe an HTTP health endpoint. Success is a 2xx status; redirects are
/// followed. Any transport failure is an unhealthy result, never an error —
/// a probe answers a question rather than raising one.
pub async fn probe_http(url: &str, probe_timeout: Option<Duration>) -> ProbeResult {
	let deadline = probe_timeout.unwrap_or(DEFAULT_TIMEOUT);
	match timeout(deadline, fetch_status(url.to_string())).await {
		Ok(Ok(status)) => ProbeResult {
			ok: (200..300).contains(&status),
			detail: format!("HTTP {status}"),
		},
		Ok(Err(detail)) => ProbeResult { ok: false, detail },
		Err(_) => ProbeResult {
			ok: false,
			detail: format!("timed out after {}ms", deadline.as_millis()),
		},
	}
}

/// Whether a pid names a live process.
#[cfg(unix)]
pub fn process_alive(pid: u32) -> bool {
	// Signal 0 performs the permission and existence checks without
	// delivering anything.
	unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(not(unix))]
pub fn process_alive(_pid: u32) -> bool {
	false
}

struct HttpTarget {
	host: String,
	port: u16,
	path: String,
}

fn parse_http_url(url: &str) -> Result<HttpTarget, String> {
	let rest = url
		.strip_prefix("http://")
		.ok_or_else(|| format!("unsupported url: {url}"))?;
	let (authority, path) = match rest.find('/') {
		Some(index) => (&rest[..index], &rest[index..]),
		None => (rest, "/"),
	};
	let (host, port) = match authority.rsplit_once(':') {
		Some((host, port)) => (
			host.to_string(),
			port.parse::<u16>().map_err(|_| format!("invalid port in {url}"))?,
		),
		None => (authority.to_string(), 80),
	};
	if host.is_empty() {
		return Err(format!("missing host in {url}"));
	}
	Ok(HttpTarget {
		host,
		port,
		path: path.to_string(),
	})
}

async fn fetch_status(mut url: String) -> Result<u16, String> {
	for _ in 0..=MAX_REDIRECTS {
		let target = parse_http_url(&url)?;
		let mut stream = TcpStream::connect((target.host.as_str(), target.port))
			.await
			.map_err(|e| e.to_string())?;
		let request = format!(
			"GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
			target.path, target.host
		);
		stream
			.write_all(request.as_bytes())
			.await
			.map_err(|e| e.to_string())?;

		let mut response = Vec::with_capacity(1024);
		let mut buffer = [0u8; 1024];
		// Only the head is needed: stop once the header terminator arrives.
		loop {
			let read = stream.read(&mut buffer).await.map_err(|e| e.to_string())?;
			if read == 0 {
				break;
			}
			response.extend_from_slice(&buffer[..read]);
			if response.windows(4).any(|w| w == b"\r\n\r\n") || response.len() > 16 * 1024 {
				break;
			}
		}
		let head = String::from_utf8_lossy(&response);
		let status_line = head.lines().next().unwrap_or_default();
		let status: u16 = status_line
			.split_whitespace()
			.nth(1)
			.and_then(|code| code.parse().ok())
			.ok_or_else(|| format!("malformed response: {status_line}"))?;

		if matches!(status, 301 | 302 | 303 | 307 | 308) {
			if let Some(location) = head
				.lines()
				.find(|line| line.to_ascii_lowercase().starts_with("location:"))
				.map(|line| line[9..].trim().to_string())
			{
				url = if location.starts_with("http://") {
					location
				} else if location.starts_with('/') {
					format!("http://{}:{}{}", target.host, target.port, location)
				} else {
					return Err(format!("unsupported redirect target: {location}"));
				};
				continue;
			}
		}
		return Ok(status);
	}
	Err("too many redirects".to_string())
}
