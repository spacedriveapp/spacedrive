//! The RPC server's trust boundary and what the daemon logs (threat model F1,
//! F2, F4 and F6).
//!
//! Each test starts a core with its RPC server in this process and talks to it
//! over raw TCP, the way any local process could.

use std::{
	io::Write,
	path::PathBuf,
	sync::{Arc, Mutex, OnceLock},
	time::Duration,
};

use sd_core::{
	infra::daemon::{auth, rpc::RpcServer},
	Core,
};
use tempfile::TempDir;
use tokio::{
	io::{AsyncReadExt, AsyncWriteExt},
	net::{TcpListener, TcpStream},
};

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
	fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
		self.0.lock().unwrap().extend_from_slice(buf);
		Ok(buf.len())
	}

	fn flush(&mut self) -> std::io::Result<()> {
		Ok(())
	}
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Captured {
	type Writer = Captured;

	fn make_writer(&'a self) -> Self::Writer {
		self.clone()
	}
}

/// Every tracing record of this test binary, captured from its first use.
fn captured_log() -> &'static Captured {
	static LOG: OnceLock<Captured> = OnceLock::new();
	LOG.get_or_init(|| {
		let captured = Captured::default();
		tracing::subscriber::set_global_default(
			tracing_subscriber::fmt()
				.with_max_level(tracing::Level::TRACE)
				.with_ansi(false)
				.with_writer(captured.clone())
				.finish(),
		)
		.unwrap();
		captured
	})
}

struct Daemon {
	_temp: TempDir,
	_core: Arc<Core>,
	addr: String,
	data_dir: PathBuf,
}

async fn start_daemon() -> Daemon {
	captured_log();
	let temp = TempDir::new().unwrap();
	let data_dir = temp.path().join("core");
	let core = Arc::new(Core::new(data_dir.clone()).await.unwrap());
	let addr = {
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		listener.local_addr().unwrap().to_string()
	};
	let server_addr = addr.clone();
	let server_core = core.clone();
	tokio::spawn(async move {
		let mut server = RpcServer::new(server_addr, server_core);
		let _ = server.start().await;
	});
	for _ in 0..100 {
		if auth::secret_path(&data_dir).exists() && TcpStream::connect(&addr).await.is_ok() {
			break;
		}
		tokio::time::sleep(Duration::from_millis(50)).await;
	}
	Daemon {
		_temp: temp,
		_core: core,
		addr,
		data_dir,
	}
}

/// Send raw bytes on a fresh connection and return everything the daemon
/// answers before it closes.
async fn exchange(addr: &str, bytes: &str) -> String {
	let mut stream = TcpStream::connect(addr).await.unwrap();
	stream.write_all(bytes.as_bytes()).await.unwrap();
	let mut answer = String::new();
	let _ = tokio::time::timeout(Duration::from_secs(5), stream.read_to_string(&mut answer)).await;
	answer
}

async fn secret(daemon: &Daemon) -> String {
	auth::read(&daemon.data_dir).await.unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn rpc_without_the_secret_is_refused() {
	let daemon = start_daemon().await;

	let answer = exchange(&daemon.addr, "\"Ping\"\n").await;
	assert!(
		answer.contains("SecurityError") && !answer.contains("Pong"),
		"a request without the secret is refused, got {answer:?}"
	);

	let wrong = auth::auth_line(&"0".repeat(64));
	let answer = exchange(&daemon.addr, &(wrong + "\"Ping\"\n")).await;
	assert!(
		answer.contains("SecurityError") && !answer.contains("Pong"),
		"a wrong secret is refused, got {answer:?}"
	);

	let right = auth::auth_line(&secret(&daemon).await);
	let answer = exchange(&daemon.addr, &(right + "\"Ping\"\n")).await;
	assert_eq!(
		answer.trim(),
		"\"Pong\"",
		"the daemon's own secret is served"
	);
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_without_the_secret_does_nothing() {
	let daemon = start_daemon().await;

	let answer = exchange(&daemon.addr, "\"Shutdown\"\n").await;
	assert!(answer.contains("SecurityError"), "got {answer:?}");
	tokio::time::sleep(Duration::from_millis(300)).await;

	let right = auth::auth_line(&secret(&daemon).await);
	let answer = exchange(&daemon.addr, &(right + "\"Ping\"\n")).await;
	assert_eq!(answer.trim(), "\"Pong\"", "the daemon is still up");
}

/// F6: an HTTP request, such as a web page could send to loopback, is never
/// dispatched.
#[tokio::test(flavor = "multi_thread")]
async fn an_http_request_is_not_dispatched() {
	let daemon = start_daemon().await;

	let request = format!(
		"POST / HTTP/1.1\r\nHost: {}\r\nContent-Type: text/plain\r\nContent-Length: 9\r\n\r\n\"Shutdown\"\n",
		daemon.addr
	);
	let answer = exchange(&daemon.addr, &request).await;
	assert!(
		!answer.starts_with("HTTP/"),
		"no HTTP answer, got {answer:?}"
	);
	assert!(answer.contains("SecurityError"), "got {answer:?}");

	let right = auth::auth_line(&secret(&daemon).await);
	let answer = exchange(&daemon.addr, &(right + "\"Ping\"\n")).await;
	assert_eq!(answer.trim(), "\"Pong\"", "the daemon is still up");
}

/// F2: neither a malformed line after the secret nor a line sent in place
/// of it reaches the log.
#[tokio::test(flavor = "multi_thread")]
async fn malformed_rpc_lines_are_not_logged() {
	let daemon = start_daemon().await;
	let right = auth::auth_line(&secret(&daemon).await);

	let token = "sdg_F2LeakCanaryToken0123456789abcdefghijk";
	let password = "hunter2-F2-canary";
	let lines = [
		format!(r#"{{"Query":{{"method":"{token}","payload":{{"password":"{password}"}}"#),
		format!(r#"{{"Ping":"{token}"}}"#),
		format!(r#"{{"Action":{{"method":"action:x","library_id":"{token}","payload":{{}}}}}}"#),
		format!("not json {token} {password}"),
	];
	for line in &lines {
		let answer = exchange(&daemon.addr, &format!("{right}{line}\n")).await;
		assert!(
			answer.contains("SerializationError"),
			"a malformed line gets a parse error, got {answer:?}"
		);
		let answer = exchange(&daemon.addr, &format!("{line}\n")).await;
		assert!(
			answer.contains("SecurityError"),
			"a line in place of the secret is refused, got {answer:?}"
		);
	}
	tokio::time::sleep(Duration::from_millis(200)).await;

	let log = String::from_utf8_lossy(&captured_log().0.lock().unwrap()).into_owned();
	assert!(
		log.contains("Failed to parse daemon request"),
		"the parse failures were logged"
	);
	assert!(
		log.contains("did not present the daemon's secret"),
		"the refusals were logged"
	);
	assert!(
		!log.contains("sdg_"),
		"a grant token reached the log:\n{log}"
	);
	assert!(
		!log.contains(password),
		"a password reached the log:\n{log}"
	);
	assert!(
		!log.contains(&secret(&daemon).await),
		"the RPC secret reached the log"
	);
}

/// F4: the daemon logs where its SMB share listens but not the login a host
/// client mounts it with.
#[tokio::test(flavor = "multi_thread")]
async fn smb_share_password_is_not_logged() {
	let _daemon = start_daemon().await;

	let mut log = String::new();
	for _ in 0..100 {
		log = String::from_utf8_lossy(&captured_log().0.lock().unwrap()).into_owned();
		if log.contains("Mounts SMB share at") {
			break;
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	}
	assert!(
		log.contains("Mounts SMB share at smb://***@127.0.0.1:"),
		"the share address was logged with its login masked:\n{log}"
	);

	let url = sd_core::service::mounts::smb::mount_url().expect("the SMB share is running");
	let password = url
		.strip_prefix("smb://spacedrive:")
		.and_then(|rest| rest.split('@').next())
		.expect("the mount URL carries the login");
	assert!(!password.is_empty(), "the mount URL carries a password");
	assert!(
		!log.contains(&format!(":{password}@")),
		"the SMB login reached the log:\n{log}"
	);
	// A pinned password can be an ordinary word found elsewhere in the log;
	// a generated one cannot.
	if std::env::var_os("SD_MOUNT_SMB_PASSWORD").is_none() {
		assert!(
			!log.contains(password),
			"the SMB password reached the log:\n{log}"
		);
	}
}
