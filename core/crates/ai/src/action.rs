use crate::{define_concept, Concept, Prompt};

#[derive(Prompt, Debug, Clone)]
#[prompt(meaning = r###"
        The system will create actions for all capabilities executed. You can review these at any time to ensure the system is behaving as expected. Ensure we are not looping or stuck in a state.
    "###)]
pub struct Action {
	pub name: String,
	pub description: String,
}

define_concept!(Action);
