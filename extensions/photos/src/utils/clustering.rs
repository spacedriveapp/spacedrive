use spacedrive_sdk::prelude::*;
use uuid::Uuid;

use crate::agent::PhotoEvent;
use crate::models::*;

pub fn dbscan_clustering(faces: &[(Uuid, FaceDetection)], threshold: f32) -> Vec<FaceCluster> {
	todo!("Implement DBSCAN clustering")
}

pub fn cluster_by_location(photos: &[Entry], radius_meters: f32) -> Vec<PlaceCluster> {
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
	pub photos: Vec<Entry>,
	pub center: GpsCoordinates,
}
