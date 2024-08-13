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
		self.prompt.push_str(
			format!(
				"\n\n### {}:\n- {}\n",
				section_name.to_uppercase(),
				prompt.generate_prompt()
			)
			.as_str(),
		);
	}

	pub fn add_section_grouped<T: Prompt>(&mut self, section_name: String, prompts: Vec<T>) {
		let mut section = format!("\n\n### {}:\n\n", section_name.to_uppercase());
		for prompt in prompts {
			section.push_str(format!("- {}\n", prompt.generate_prompt()).as_str());
		}
		self.prompt.push_str(section.as_str());
	}

	pub fn add_text_section(&mut self, section_name: String, text: String) {
		self.prompt
			.push_str(format!("\n\n### {}:\n- {}\n", section_name.to_uppercase(), text).as_str());
	}

	pub fn finalize(&self) -> String {
		let mut finalized_prompt = self.prompt.clone();
		finalized_prompt.push_str("\n");
		finalized_prompt
	}
}
