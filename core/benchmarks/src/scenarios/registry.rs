use super::{ContentIdentificationScenario, CoreIndexingScenario, Scenario};

pub fn registered_scenarios() -> Vec<Box<dyn Scenario>> {
	vec![
		Box::new(CoreIndexingScenario::default()),
		Box::new(ContentIdentificationScenario::default()),
	]
}
