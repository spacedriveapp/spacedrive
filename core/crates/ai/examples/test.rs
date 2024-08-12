use chrono::Utc;
use sd_core_ai::{
	concept::{register_concept, AnyConceptWrapper, CONCEPT_REGISTRY},
	journal::Journal,
	objective::Objective,
	system_prompt::{ProcessConfig, SystemPrompt},
	Concept, Prompt,
};

fn main() {
	println!("Running an example!");

	// Registering concepts
	register_concept::<Objective>();
	register_concept::<Journal>();

	// Creating an objective
	let objective = Objective {
		description: "This is a test objective".to_string(),
		complete: false,
		active: true,
		due: None,
		priority: 5,
		relevant_concepts: vec![],
		relevant_capabilities: vec![],
	};

	// Creating a journal
	let journal = Journal { entries: vec![] };

	// Storing the concepts
	objective.store();
	journal.store();

	// Setting up the system prompt
	let mut system_prompt = SystemPrompt::new();

	// Register the concepts in the system prompt
	system_prompt.register_concept(objective);
	system_prompt.register_concept(journal);

	// Optionally add a process configuration
	let process_config = ProcessConfig {
		required_concepts: vec!["Objective", "Journal"],
		required_capabilities: vec![],
	};
	system_prompt.add_process(process_config);

	// Generate the first system prompt with base instructions and concept directory
	let prompt = system_prompt.generate_prompt();
	println!("Generated System Prompt:\n{}", prompt);

	// Optional: Validate the generated prompt (would typically be done in a test)
	assert!(prompt.contains("Welcome to the AI System"));
	assert!(prompt.contains("Objective"));
	assert!(prompt.contains("Journal"));
	assert!(prompt.contains("Required Concepts: [\"Objective\", \"Journal\"]"));
	assert!(prompt.contains("Base Instructions"));
	assert!(prompt.contains("Registered Concepts Directory"));
}
