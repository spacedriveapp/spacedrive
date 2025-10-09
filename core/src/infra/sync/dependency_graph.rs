//! Dependency graph for sync model ordering
//!
//! Computes the topological sort order for syncing models based on their foreign key dependencies.
//! This ensures that parent records always sync before child records, preventing FK violations.

use std::collections::{HashMap, HashSet, VecDeque};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DependencyError {
	#[error("Circular dependency detected: {0}")]
	CircularDependency(String),

	#[error("Unknown dependency: model '{0}' depends on '{1}' which is not registered")]
	UnknownDependency(String, String),

	#[error("No models registered")]
	NoModels,
}

/// Build a dependency graph and compute topological sort order
///
/// # Arguments
/// - `models`: Iterator of (model_name, dependencies) tuples
///
/// # Returns
/// Ordered list of model names where dependencies always come before dependents
///
/// # Example
/// ```ignore
/// let models = vec![
///     ("entry", &["location"][..]),
///     ("location", &["device"][..]),
///     ("device", &[][..]),
///     ("tag", &[][..]),
/// ];
/// let order = compute_sync_order(models.into_iter())?;
/// // order = ["device", "location", "entry", "tag"]
/// // or    = ["device", "tag", "location", "entry"]
/// // (tag and device are independent, so order between them doesn't matter)
/// ```
pub fn compute_sync_order<'a>(
	models: impl Iterator<Item = (&'a str, &'a [&'a str])>,
) -> Result<Vec<String>, DependencyError> {
	let mut graph: HashMap<String, Vec<String>> = HashMap::new();
	let mut in_degree: HashMap<String, usize> = HashMap::new();
	let mut all_models: HashSet<String> = HashSet::new();

	// Build the graph
	for (model, deps) in models {
		let model = model.to_string();
		all_models.insert(model.clone());

		// Initialize in-degree if not present
		in_degree.entry(model.clone()).or_insert(0);

		// Add edges for each dependency
		for dep in deps.iter() {
			let dep = dep.to_string();

			// Track all referenced models
			all_models.insert(dep.clone());

			// Add edge: dep -> model (dep must come before model)
			graph
				.entry(dep.clone())
				.or_insert_with(Vec::new)
				.push(model.clone());

			// Increment in-degree for this model
			*in_degree.entry(model.clone()).or_insert(0) += 1;

			// Initialize in-degree for dependency if not present
			in_degree.entry(dep).or_insert(0);
		}
	}

	if all_models.is_empty() {
		return Err(DependencyError::NoModels);
	}

	// Validate that all dependencies are registered models
	for (model, deps) in graph.iter() {
		for dep in deps {
			if !all_models.contains(dep) {
				return Err(DependencyError::UnknownDependency(
					dep.clone(),
					model.clone(),
				));
			}
		}
	}

	// Kahn's algorithm for topological sort
	let mut queue: VecDeque<String> = in_degree
		.iter()
		.filter(|(_, &degree)| degree == 0)
		.map(|(model, _)| model.clone())
		.collect();

	let mut result = Vec::new();

	while let Some(model) = queue.pop_front() {
		result.push(model.clone());

		// For each dependent of this model
		if let Some(dependents) = graph.get(&model) {
			for dependent in dependents {
				// Decrease in-degree
				if let Some(degree) = in_degree.get_mut(dependent) {
					*degree -= 1;
					if *degree == 0 {
						queue.push_back(dependent.clone());
					}
				}
			}
		}
	}

	// Check for cycles
	if result.len() != all_models.len() {
		// Find the models that couldn't be sorted (part of a cycle)
		let sorted: HashSet<_> = result.iter().cloned().collect();
		let unsorted: Vec<_> = all_models
			.iter()
			.filter(|m| !sorted.contains(*m))
			.cloned()
			.collect();

		return Err(DependencyError::CircularDependency(format!(
			"Models involved in cycle: {}",
			unsorted.join(", ")
		)));
	}

	Ok(result)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_simple_dependency_chain() {
		let models = vec![
			("entry", &["location"][..]),
			("location", &["device"][..]),
			("device", &[][..]),
		];

		let order = compute_sync_order(models.into_iter()).unwrap();

		// Device must come before location
		let device_idx = order.iter().position(|m| m == "device").unwrap();
		let location_idx = order.iter().position(|m| m == "location").unwrap();
		let entry_idx = order.iter().position(|m| m == "entry").unwrap();

		assert!(device_idx < location_idx);
		assert!(location_idx < entry_idx);
	}

	#[test]
	fn test_independent_models() {
		let models = vec![("device", &[][..]), ("tag", &[][..])];

		let order = compute_sync_order(models.into_iter()).unwrap();

		// Both models should be present
		assert_eq!(order.len(), 2);
		assert!(order.contains(&"device".to_string()));
		assert!(order.contains(&"tag".to_string()));
		// Order between them doesn't matter
	}

	#[test]
	fn test_circular_dependency() {
		let models = vec![("a", &["b"][..]), ("b", &["a"][..])];

		let result = compute_sync_order(models.into_iter());
		assert!(matches!(
			result,
			Err(DependencyError::CircularDependency(_))
		));
	}

	#[test]
	fn test_complex_graph() {
		// More realistic dependency graph
		let models = vec![
			("entry", &["location"][..]),
			("location", &["device"][..]),
			("device", &[][..]),
			("tag", &[][..]),
			("tag_relationship", &["tag"][..]),
		];

		let order = compute_sync_order(models.into_iter()).unwrap();

		// Verify all models are present
		assert_eq!(order.len(), 5);

		// Device must come before location
		let device_idx = order.iter().position(|m| m == "device").unwrap();
		let location_idx = order.iter().position(|m| m == "location").unwrap();
		assert!(device_idx < location_idx);

		// Location must come before entry
		let entry_idx = order.iter().position(|m| m == "entry").unwrap();
		assert!(location_idx < entry_idx);

		// Tag must come before tag_relationship
		let tag_idx = order.iter().position(|m| m == "tag").unwrap();
		let tag_rel_idx = order.iter().position(|m| m == "tag_relationship").unwrap();
		assert!(tag_idx < tag_rel_idx);
	}

	#[test]
	fn test_no_models() {
		let models: Vec<(&str, &[&str])> = vec![];
		let result = compute_sync_order(models.into_iter());
		assert!(matches!(result, Err(DependencyError::NoModels)));
	}
}
