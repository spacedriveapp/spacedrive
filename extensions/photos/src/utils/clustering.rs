use spacedrive_sdk::prelude::*;
use uuid::Uuid;

use crate::agent::PhotoEvent;
use crate::models::{FaceDetection, *};

/// Cosine distance between two embeddings: 0 for identical directions, 1
/// for orthogonal ones. Vectors of different lengths or zero length are as
/// far apart as possible.
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
	if a.len() != b.len() || a.is_empty() {
		return 1.0;
	}
	let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
	let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
	let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
	if norm_a == 0.0 || norm_b == 0.0 {
		return 1.0;
	}
	1.0 - dot / (norm_a * norm_b)
}

/// The element-wise mean of a set of embeddings; empty when the set is.
pub fn mean_embedding(embeddings: &[Vec<f32>]) -> Vec<f32> {
	let Some(first) = embeddings.first() else {
		return Vec::new();
	};
	let mut mean = vec![0.0f32; first.len()];
	let mut count = 0usize;
	for embedding in embeddings.iter().filter(|e| e.len() == first.len()) {
		for (m, x) in mean.iter_mut().zip(embedding) {
			*m += x;
		}
		count += 1;
	}
	for m in &mut mean {
		*m /= count as f32;
	}
	mean
}

/// DBSCAN over face embeddings with cosine distance, `min_points` of 2:
/// a face with no neighbour within `threshold` is noise and belongs to no
/// person. Quadratic in the number of faces, which is fine for one job's
/// batch and avoids an index the guest has no room for.
pub fn dbscan_clustering(faces: &[(Uuid, FaceDetection)], threshold: f32) -> Vec<FaceCluster> {
	const MIN_POINTS: usize = 2;
	let n = faces.len();
	let neighbours: Vec<Vec<usize>> = (0..n)
		.map(|i| {
			(0..n)
				.filter(|&j| {
					j != i
						&& cosine_distance(&faces[i].1.embedding, &faces[j].1.embedding)
							<= threshold
				})
				.collect()
		})
		.collect();

	let mut cluster_of = vec![None::<usize>; n];
	let mut clusters: Vec<Vec<usize>> = Vec::new();
	for seed in 0..n {
		if cluster_of[seed].is_some() || neighbours[seed].len() + 1 < MIN_POINTS {
			continue;
		}
		let id = clusters.len();
		let mut members = Vec::new();
		let mut frontier = vec![seed];
		cluster_of[seed] = Some(id);
		while let Some(i) = frontier.pop() {
			members.push(i);
			if neighbours[i].len() + 1 < MIN_POINTS {
				continue;
			}
			for &j in &neighbours[i] {
				if cluster_of[j].is_none() {
					cluster_of[j] = Some(id);
					frontier.push(j);
				}
			}
		}
		clusters.push(members);
	}

	clusters
		.into_iter()
		.map(|members| {
			let faces: Vec<(Uuid, FaceDetection)> =
				members.iter().map(|&i| faces[i].clone()).collect();
			let embeddings: Vec<Vec<f32>> =
				faces.iter().map(|(_, f)| f.embedding.clone()).collect();
			FaceCluster {
				centroid_embedding: mean_embedding(&embeddings),
				faces,
			}
		})
		.collect()
}

pub fn cluster_by_location(photos: &[Record], radius_meters: f32) -> Vec<PlaceCluster> {
	todo!("Implement geographic clustering")
}

pub fn cluster_into_moments(events: &[PhotoEvent]) -> Vec<MomentGroup> {
	todo!("Implement moment clustering")
}

pub struct FaceCluster {
	pub faces: Vec<(Uuid, FaceDetection)>,
	pub centroid_embedding: Vec<f32>,
}

pub struct PlaceCluster {
	pub photos: Vec<Record>,
	pub center: GpsCoordinates,
}
