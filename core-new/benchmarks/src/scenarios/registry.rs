use super::{IndexingDiscoveryScenario, Scenario};

pub fn registered_scenarios() -> Vec<Box<dyn Scenario>> {
    vec![Box::new(IndexingDiscoveryScenario::default())]
}
