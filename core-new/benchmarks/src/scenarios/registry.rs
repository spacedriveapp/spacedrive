use super::{IndexingDiscoveryScenario, Scenario};
use crate::scenarios::aggregation::AggregationScenario;
use crate::scenarios::content_identification::ContentIdentificationScenario;

pub fn registered_scenarios() -> Vec<Box<dyn Scenario>> {
	vec![
		Box::new(IndexingDiscoveryScenario::default()),
		Box::new(ContentIdentificationScenario::default()),
		Box::new(AggregationScenario::default()),
	]
}
