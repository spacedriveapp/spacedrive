//! A malformed RPC line never reaches the log (threat model F2).
//!
//! Starts a core with its RPC server in this process, captures every tracing
//! record into memory, sends lines that fail to parse while carrying a grant
//! token and a password, and checks that neither shows up in what was logged.

use std::{
	io::Write,
	sync::{Arc, Mutex},
	time::Duration,
};

use sd_core::{infra::daemon::rpc::RpcServer, Core};
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

async fn free_addr() -> String {
	let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
	listener.local_addr().unwrap().to_string()
}

async fn send_line(addr: &str, line: &str) -> String {
	let mut stream = TcpStream::connect(addr).await.unwrap();
	stream.write_all(line.as_bytes()).await.unwrap();
	stream.write_all(b"\n").await.unwrap();
	let mut answer = String::new();
	let _ = tokio::time::timeout(Duration::from_secs(5), stream.read_to_string(&mut answer)).await;
	answer
}

#[tokio::test(flavor = "multi_thread")]
async fn malformed_rpc_lines_are_not_logged() {
	let captured = Captured::default();
	tracing::subscriber::set_global_default(
		tracing_subscriber::fmt()
			.with_max_level(tracing::Level::TRACE)
			.with_ansi(false)
			.with_writer(captured.clone())
			.finish(),
	)
	.unwrap();

	let temp = TempDir::new().unwrap();
	let core = Arc::new(Core::new(temp.path().join("core")).await.unwrap());
	let addr = free_addr().await;
	let server_addr = addr.clone();
	let server_core = core.clone();
	tokio::spawn(async move {
		let mut server = RpcServer::new(server_addr, server_core);
		let _ = server.start().await;
	});
	for _ in 0..50 {
		if TcpStream::connect(&addr).await.is_ok() {
			break;
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	}

	let token = "sdg_F2LeakCanaryToken0123456789abcdefghijk";
	let password = "hunter2-F2-canary";
	let lines = [
		format!(r#"{{"Query":{{"method":"{token}","payload":{{"password":"{password}"}}"#),
		format!(r#"{{"Ping":"{token}"}}"#),
		format!(r#"{{"Action":{{"method":"action:x","library_id":"{token}","payload":{{}}}}}}"#),
		format!("not json {token} {password}"),
	];
	for line in &lines {
		let answer = send_line(&addr, line).await;
		assert!(
			answer.contains("Error"),
			"a malformed line gets an error answer, got {answer:?}"
		);
	}
	tokio::time::sleep(Duration::from_millis(200)).await;

	let log = String::from_utf8_lossy(&captured.0.lock().unwrap()).into_owned();
	assert!(
		log.contains("Failed to parse daemon request"),
		"the parse failures were logged"
	);
	assert!(
		!log.contains("sdg_"),
		"a grant token reached the log:\n{log}"
	);
	assert!(
		!log.contains(password),
		"a password reached the log:\n{log}"
	);
}
