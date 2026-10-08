use chrono::{DateTime, Utc};
use spacedrive_sdk::prelude::*;
use uuid::Uuid;

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

/// Great-circle distance in metres between two coordinates.
pub fn distance_meters(a: &GpsCoordinates, b: &GpsCoordinates) -> f64 {
	const EARTH_RADIUS_M: f64 = 6_371_000.0;
	let (lat_a, lat_b) = (a.latitude.to_radians(), b.latitude.to_radians());
	let d_lat = lat_b - lat_a;
	let d_lon = (b.longitude - a.longitude).to_radians();
	let h = (d_lat / 2.0).sin().powi(2) + lat_a.cos() * lat_b.cos() * (d_lon / 2.0).sin().powi(2);
	2.0 * EARTH_RADIUS_M * h.sqrt().asin()
}

/// Greedy clustering by place: a photo joins the first cluster whose
/// centre is within `radius_meters`, else starts one. The centre is the
/// running mean of its members, which is enough for a place a few hundred
/// metres across and avoids a second pass.
pub fn cluster_by_location(
	photos: &[(Record, GpsCoordinates)],
	radius_meters: f64,
) -> Vec<PlaceCluster> {
	let mut clusters: Vec<PlaceCluster> = Vec::new();
	for (photo, coords) in photos {
		match clusters
			.iter_mut()
			.find(|c| distance_meters(&c.center, coords) <= radius_meters)
		{
			Some(cluster) => {
				let n = cluster.photos.len() as f64;
				cluster.center.latitude = (cluster.center.latitude * n + coords.latitude) / (n + 1.0);
				cluster.center.longitude =
					(cluster.center.longitude * n + coords.longitude) / (n + 1.0);
				cluster.photos.push(photo.clone());
			}
			None => clusters.push(PlaceCluster {
				photos: vec![photo.clone()],
				center: coords.clone(),
			}),
		}
	}
	clusters
}

/// One photo's capture facts, as EXIF gave them.
#[derive(Clone)]
pub struct PhotoCapture {
	pub photo_id: Uuid,
	pub taken: DateTime<Utc>,
	pub location: Option<GpsCoordinates>,
}

/// Split captures, in time order, wherever neighbours are more than
/// `gap_hours` apart or, when both have a location, more than `split_km`
/// apart. A moment is one such run.
pub fn cluster_into_moments(
	captures: &[PhotoCapture],
	gap_hours: f64,
	split_km: f64,
) -> Vec<MomentGroup> {
	let mut ordered: Vec<&PhotoCapture> = captures.iter().collect();
	ordered.sort_by_key(|c| (c.taken, c.photo_id));

	let mut groups: Vec<MomentGroup> = Vec::new();
	let mut previous: Option<&PhotoCapture> = None;
	for capture in ordered {
		let starts_new = match previous {
			None => true,
			Some(last) => {
				let hours = (capture.taken - last.taken).num_seconds() as f64 / 3600.0;
				let far = match (&last.location, &capture.location) {
					(Some(a), Some(b)) => distance_meters(a, b) > split_km * 1000.0,
					_ => false,
				};
				hours > gap_hours || far
			}
		};
		if starts_new {
			groups.push(MomentGroup {
				photo_ids: Vec::new(),
				start_date: capture.taken,
				end_date: capture.taken,
				place_id: None,
				place_name: None,
				common_scenes: Vec::new(),
			});
		}
		let group = groups.last_mut().expect("pushed above");
		group.photo_ids.push(capture.photo_id);
		group.end_date = capture.taken;
		previous = Some(capture);
	}
	groups
}

pub struct FaceCluster {
	pub faces: Vec<(Uuid, FaceDetection)>,
	pub centroid_embedding: Vec<f32>,
}

pub struct PlaceCluster {
	pub photos: Vec<Record>,
	pub center: GpsCoordinates,
}
