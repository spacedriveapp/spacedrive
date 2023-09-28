pub struct Env {
	pub api_url: String,
}

impl Default for Env {
	fn default() -> Self {
		Self::new()
	}
}

impl Env {
	pub fn new() -> Self {
		Self {
			api_url: std::env::var("SD_API_URL").expect("Env var 'SD_API_URL' missing!"),
		}
	}
}
