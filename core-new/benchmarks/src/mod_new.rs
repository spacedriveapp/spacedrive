pub use crate::{
    cli, config, core_boot, generator, metrics, recipe, reporting, runner, scenarios, util,
};

pub fn list_scenarios() -> Vec<&'static str> {
	scenarios::registry::registered_scenarios()
		.iter()
		.map(|s| s.name())
		.collect()
}

pub fn list_reporters() -> Vec<&'static str> {
	reporting::registry::registered_reporters()
		.iter()
		.map(|r| r.name())
		.collect()
}

pub fn list_generators() -> Vec<&'static str> {
	generator::registry::registered_generators()
		.iter()
		.map(|g| g.name())
		.collect()
}
