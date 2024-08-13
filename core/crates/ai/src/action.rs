use crate::{define_concept, Concept, Prompt};

#[derive(Prompt, Debug, Clone)]
#[prompt(instruct = r###"
        The System will create actions for all capabilities executed. You can review these at any time to ensure the system is behaving as expected. Ensure we are not looping or stuck in a state.
    "###,
)]
pub struct Action {
	pub name: String,
	pub description: String,
	pub timestamp: chrono::DateTime<chrono::Utc>,
	pub execution_time: chrono::Duration,
	pub status: ActionStatus,
}

#[derive(Prompt, Debug, Clone)]
pub enum ActionStatus {
	Success,
	Failure,
	Timeout,
}

define_concept!(Action);
