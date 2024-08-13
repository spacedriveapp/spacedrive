use crate::instruct::BASE_INSTRUCT;

pub trait Prompt {
	fn generate_prompt(&self) -> String;
}

#[derive(Debug)]
pub struct PromptFactory {
	prompt: String,
}

impl PromptFactory {
	pub fn new() -> Self {
		Self {
			prompt: BASE_INSTRUCT.to_string(),
		}
	}
	pub fn add_section<T: Prompt>(&mut self, section_name: String, prompt: &T) {
		self.prompt
			.push_str(format!("\n\n### {}: \n{}", section_name, prompt.generate_prompt()).as_str());
	}

	pub fn add_section_grouped<T: Prompt>(&mut self, section_name: String, prompts: Vec<T>) {
		let mut section = format!("\n\n### {}:\n\n", section_name);
		for prompt in prompts {
			section.push_str(format!("{}\n", prompt.generate_prompt()).as_str());
		}
		self.prompt.push_str(section.as_str());
	}

	pub fn add_text_section_grouped(&mut self, section_name: String, prompts: Vec<String>) {
		let mut section = format!("\n\n### {}:", section_name);
		for prompt in prompts {
			section.push_str(format!("{}\n\n", prompt).as_str());
		}
		self.prompt.push_str(section.as_str());
	}

	pub fn finalize(&self) -> String {
		self.prompt.clone()
	}
}
