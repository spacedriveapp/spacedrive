use serde::{Deserialize, Serialize};

use crate::{concept::*, define_concept, Prompt, SchemaProvider};
use schemars::{schema_for, JsonSchema};
#[derive(Prompt, JsonSchema, Debug, Clone, Default, Serialize, Deserialize)]
#[prompt(instruct = r###"
        The System will create actions for all capabilities executed. You can review these at any time to ensure the system is behaving as expected. Ensure we are not looping or stuck in a state.
    "###)]
pub struct Action {
	pub name: String,
	pub description: String,

	// pub timestamp: chrono::DateTime<chrono::Utc>,
	// pub execution_time: chrono::Duration,
	pub status: ActionStatus,
}

define_concept!(Action);

#[derive(Prompt, Debug, Clone, Default, JsonSchema, Serialize, Deserialize)]
#[prompt(instruct = r###"
        The status of the action.
    "###)]
pub enum ActionStatus {
	#[default]
	Success,
	Failure,
	Timeout,
}

define_concept!(ActionStatus);
