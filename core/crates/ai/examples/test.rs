use sd_core_ai::ModelInstance;

#[tokio::main]
async fn main() {
	println!("Starting model instance...");
	let mut model = ModelInstance::new();

	// The concepts are already registered within the new method
	// let concepts = sd_core_ai::concept::list_concepts();
	// println!("Registered Concepts: {:?}", concepts);

	model.generate_system_prompt();

	let _ = model.start().await;
}
