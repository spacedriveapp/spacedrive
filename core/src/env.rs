use tokio::sync::Mutex;

pub struct Env {
	pub api_url: Mutex<String>,
	pub client_id: String,
}

impl Env {
	pub fn new(client_id: &str) -> Self {
		Self {
			api_url: Mutex::new("https://app.spacedrive.com".to_string()),
			client_id: client_id.to_string(),
		}
	}
}
