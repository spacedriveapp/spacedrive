use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::infra::daemon::types::{DaemonRequest, DaemonResponse};
use crate::infra::event::Event;
use crate::infra::event::log_emitter::LogMessage;

#[derive(Clone)]
pub struct DaemonClient {
	socket_addr: String,
}

impl DaemonClient {
	pub fn new(socket_addr: String) -> Self {
		Self { socket_addr }
	}

	pub async fn send(
		&self,
		req: &DaemonRequest,
	) -> Result<DaemonResponse, Box<dyn std::error::Error + Send + Sync>> {
		let mut stream = TcpStream::connect(&self.socket_addr).await.map_err(|e| {
			format!(
				"Failed to connect to daemon at {}: {}",
				self.socket_addr,
				e
			)
		})?;

		let payload =
			serde_json::to_vec(req).map_err(|e| format!("Failed to serialize request: {}", e))?;

		stream
			.write_all(&payload)
			.await
			.map_err(|e| format!("Failed to send request to daemon: {}", e))?;

		stream
			.shutdown()
			.await
			.map_err(|e| format!("Failed to shutdown write stream: {}", e))?;

		let mut buf = Vec::new();
		stream
			.read_to_end(&mut buf)
			.await
			.map_err(|e| format!("Failed to read response from daemon: {}", e))?;

		let response: DaemonResponse = serde_json::from_slice(&buf)
			.map_err(|e| format!("Failed to deserialize daemon response: {}", e))?;

		Ok(response)
	}

	/// Start a streaming connection for real-time events
	pub async fn stream(
		&self,
		request: &DaemonRequest,
		event_tx: mpsc::UnboundedSender<Event>,
	) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
		let mut stream = TcpStream::connect(&self.socket_addr).await.map_err(|e| {
			format!(
				"Failed to connect to daemon at {}: {}",
				self.socket_addr,
				e
			)
		})?;

		// Send subscription request
		let payload = serde_json::to_string(request)
			.map_err(|e| format!("Failed to serialize request: {}", e))?;

		stream
			.write_all((payload + "\n").as_bytes())
			.await
			.map_err(|e| format!("Failed to send request to daemon: {}", e))?;

		// Split stream for reading responses
		let (reader, _writer) = stream.into_split();
		let mut buf_reader = BufReader::new(reader);
		let mut line = String::new();

		// Read streaming responses
		loop {
			line.clear();
			match buf_reader.read_line(&mut line).await {
				Ok(0) => break, // EOF
				Ok(_) => {
					if let Ok(response) = serde_json::from_str::<DaemonResponse>(&line.trim()) {
						match response {
							DaemonResponse::Event(event) => {
								if event_tx.send(event).is_err() {
									break; // Receiver dropped
								}
							}
							DaemonResponse::Subscribed => {
								// Subscription confirmed, continue listening
							}
							DaemonResponse::Error(e) => {
								eprintln!("Daemon error: {}", e);
								break;
							}
							_ => {
								// Unexpected response type
								break;
							}
						}
					}
				}
				Err(_) => break, // Connection error
			}
		}

		Ok(())
	}

	/// Start a streaming connection for real-time log messages
	pub async fn stream_logs(
		&self,
		request: &DaemonRequest,
		log_tx: mpsc::UnboundedSender<LogMessage>,
	) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
		let mut stream = TcpStream::connect(&self.socket_addr).await.map_err(|e| {
			format!(
				"Failed to connect to daemon at {}: {}",
				self.socket_addr,
				e
			)
		})?;

		// Send subscription request
		let payload = serde_json::to_string(request)
			.map_err(|e| format!("Failed to serialize request: {}", e))?;

		stream
			.write_all((payload + "\n").as_bytes())
			.await
			.map_err(|e| format!("Failed to send request to daemon: {}", e))?;

		// Split stream for reading responses
		let (reader, _writer) = stream.into_split();
		let mut buf_reader = BufReader::new(reader);
		let mut line = String::new();

		// Read streaming responses
		loop {
			line.clear();
			match buf_reader.read_line(&mut line).await {
				Ok(0) => break, // EOF
				Ok(_) => {
					if let Ok(response) = serde_json::from_str::<DaemonResponse>(&line.trim()) {
						match response {
							DaemonResponse::LogMessage(log_msg) => {
								if log_tx.send(log_msg).is_err() {
									break; // Receiver dropped
								}
							}
							DaemonResponse::LogsSubscribed => {
								// Subscription confirmed, continue listening
							}
							DaemonResponse::Error(e) => {
								eprintln!("Daemon error: {}", e);
								break;
							}
							_ => {
								// Unexpected response type
								break;
							}
						}
					}
				}
				Err(_) => break, // Connection error
			}
		}

		Ok(())
	}
}
