use sd_core::client::CoreClient;

#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
	Human,
	Json,
}

pub struct Context {
	pub core: CoreClient,
	pub format: OutputFormat,
}

impl Context {
	pub fn new(core: CoreClient, format: OutputFormat) -> Self {
		Self { core, format }
	}
}
