use futures::StreamExt;
use reqwest::{Client, StatusCode};
use thiserror::Error;
use tokio::sync::oneshot;
use tokio::task;

#[derive(Error, Debug)]
pub enum OllamaError {
	#[error("Failed to connect to Ollama API: {0}")]
	ConnectionError(#[from] reqwest::Error),

	#[error("Ollama API is not running on the system.")]
	ServiceUnavailable,

	#[error("Prompt processing failed with status code: {0}")]
	PromptFailed(StatusCode),

	#[error("Stream was cancelled.")]
	StreamCancelled,
}

pub struct OllamaClient {
	http_client: Client,
	base_url: String,
}

impl OllamaClient {
	pub fn new(base_url: &str) -> Self {
		Self {
			http_client: Client::new(),
			base_url: base_url.to_string(),
		}
	}

	pub async fn stream_prompt(
		&self,
		prompt: &str,
	) -> Result<impl futures::Stream<Item = String>, OllamaError> {
		let url = format!("{}/prompt", self.base_url);
		let request = self.http_client.post(&url).body(prompt.to_string());

		let response = request.send().await.map_err(|err| {
			if err.is_connect() {
				OllamaError::ServiceUnavailable
			} else {
				OllamaError::ConnectionError(err)
			}
		})?;

		if !response.status().is_success() {
			return Err(OllamaError::PromptFailed(response.status()));
		}

		let stream = response.bytes_stream();

		Ok(stream.map(|chunk| {
			let chunk = chunk.unwrap_or_else(|_| vec![].into());
			String::from_utf8_lossy(&chunk).to_string()
		}))
	}

	pub async fn stream_prompt_with_cancellation(
		&self,
		prompt: &str,
	) -> Result<
		(
			impl futures::Stream<Item = Result<String, OllamaError>>,
			oneshot::Sender<()>,
		),
		OllamaError,
	> {
		let (cancel_tx, cancel_rx) = oneshot::channel::<()>();

		let mut stream = self.stream_prompt(prompt).await?;

		let cancellable_stream = async_stream::stream! {
			tokio::select! {
				_ = cancel_rx => {
					yield Err(OllamaError::StreamCancelled);
				}
				item = stream.next() => {
					if let Some(chunk) = item {
						yield Ok(chunk);
					}
				}
			}
		};

		Ok((cancellable_stream, cancel_tx))
	}
}
