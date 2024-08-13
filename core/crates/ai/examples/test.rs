use sd_core_ai::{
	action::Action, capability::register_capability, concept::register_concept, journal::Journal,
	model::ModelEvent, objective::Objective, ModelInstance,
};

#[tokio::main]
async fn main() {
	println!("Starting model instance...");
	let mut model = ModelInstance::new();

	// let objective = Objective {
	// 	description: "This is a test objective".to_string(),
	// 	due: None,
	// 	complete: false,
	// 	active: true,
	// 	priority: 5,
	// };

	register_concept::<Objective>();
	register_concept::<Action>();
	register_concept::<Journal>();
	register_concept::<ModelEvent>();

	model.generate_system_prompt();

	let _ = model.start().await;
}
