use futures::Stream;
use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::Ollama;
use std::pin::Pin;
use tokio::sync::oneshot;

pub struct OllamaClientWrapper {
	client: Ollama,
}

impl OllamaClientWrapper {
	pub fn new(base_url: &str, port: u16) -> Self {
		Self {
			client: Ollama::new(base_url.to_string(), port),
		}
	}

	pub async fn generate_response(
		&self,
		prompt: &str,
	) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
		let model = "llama3.1:8b".to_string();
		let res = self
			.client
			.generate(GenerationRequest::new(model, prompt.to_string()))
			.await?;
		Ok(res.response)
	}

	pub async fn generate_response_stream(
		&self,
		prompt: &str,
	) -> Result<
		(
			Pin<
				Box<
					dyn Stream<Item = Result<String, Box<dyn std::error::Error + Send + Sync>>>
						+ Send,
				>,
			>,
			oneshot::Sender<()>,
		),
		Box<dyn std::error::Error + Send + Sync>,
	> {
		let (cancel_tx, cancel_rx) = oneshot::channel::<()>();

		let model = "llama3.1:8b".to_string();
		let response = self
			.client
			.generate(GenerationRequest::new(model, prompt.to_string()))
			.await?;

		let stream = async_stream::stream! {
			tokio::select! {
				_ = cancel_rx => {
					yield Err("Stream cancelled".into());
				}
				_ = async {} => {
					yield Ok(response.response);
				}
			}
		};

		Ok((Box::pin(stream), cancel_tx))
	}
}
