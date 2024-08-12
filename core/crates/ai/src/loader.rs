use chrono::prelude::*;
use sd_prompt_derive::Prompt;

enum DataType {
	Text,
	Image,
	Audio,
	Video,
	File,
}

#[derive(Prompt)]
struct DataSource;
