use sd_core_ai::{
	model::{ModelEvent, ModelEventType},
	ModelInstance,
};
use tokio::{
	io::{self, AsyncBufReadExt},
	sync::mpsc,
};

#[tokio::main]
async fn main() {
	println!("Starting model instance...");

	let mut model_instance = ModelInstance::new();

	// Clone the event sender to be used in the input handling task
	let event_tx = model_instance.event_tx.clone();

	// Spawn a task to handle user input from the console
	tokio::spawn(async move {
		let stdin = io::stdin();
		let mut reader = io::BufReader::new(stdin).lines();

		while let Ok(Some(line)) = reader.next_line().await {
			if let Err(e) = event_tx
				.send(ModelEvent {
					r#type: ModelEventType::UserMessage(line),
					text: "User input received".to_string(),
					timestamp: chrono::Utc::now().to_rfc3339(),
				})
				.await
			{
				eprintln!("Failed to send user message event: {}", e);
			}
		}
	});

	// Start the model instance processing loop
	model_instance.start().await;
}
