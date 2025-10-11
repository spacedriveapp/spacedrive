//! Photos Extension for Spacedrive
//!
//! Mirrors Apple Photos and Google Photos capabilities:
//! - Automatic face detection and clustering
//! - Place identification from GPS/EXIF
//! - Moment generation (time + location clustering)
//! - Smart search with scene understanding
//! - Memories and featured photos
//! - Albums and shared albums
//!
//! This demonstrates the full VDFS SDK specification.

#![allow(dead_code, unused_variables, unused_imports)]

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use spacedrive_sdk::prelude::*;
use spacedrive_sdk::{action, agent, agent_memory, extension, job, model};
use std::collections::HashMap;
use uuid::Uuid;

//==============================================================================
// Extension Definition
//==============================================================================

#[extension(
    id = "com.spacedrive.photos",
    name = "Photos",
    version = "1.0.0",
    description = "Advanced photo management with faces, places, and intelligent organization",
    min_core_version = "2.0.0",
    required_features = ["exif_extraction", "ai_models"],
    permissions = [
        Permission::ReadEntries,
        Permission::ReadSidecars(kinds = vec!["exif", "thumbnail"]),
        Permission::WriteSidecars(kinds = vec!["faces", "places", "scene"]),
        Permission::WriteTags,
        Permission::WriteCustomFields(namespace = "photos"),
        Permission::UseModel(category = "face_detection", preference = ModelPreference::LocalOnly),
        Permission::UseModel(category = "scene_classification", preference = ModelPreference::LocalOnly),
        Permission::DispatchJobs,
    ]
)]
struct Photos {
	config: PhotosConfig,
}

#[derive(Serialize, Deserialize)]
struct PhotosConfig {
	#[setting(label = "Enable Face Recognition", default = true)]
	face_recognition: bool,

	#[setting(label = "Enable Place Identification", default = true)]
	place_identification: bool,

	#[setting(label = "Automatically Create Memories", default = true)]
	auto_memories: bool,

	#[setting(label = "Scene Detection Confidence", default = 0.7)]
	scene_confidence_threshold: f32,

	#[setting(label = "Face Clustering Threshold", default = 0.6)]
	face_clustering_threshold: f32,
}

//==============================================================================
// Data Models
//==============================================================================

#[derive(Serialize, Deserialize, Clone)]
#[model(version = "1.0.0")]
struct Photo {
	id: Uuid,

	/// References the physical image file
	#[entry(filter = "*.{jpg,jpeg,png,heic,heif,raw,cr2,nef,dng}")]
	file: Entry,

	/// Core-extracted EXIF data (automatically available)
	#[metadata]
	exif: Option<ExifData>,

	/// Extension-owned sidecars (stored in VSS/extensions/photos/)
	#[sidecar(kind = "faces", extension_owned)]
	detected_faces: Option<Vec<FaceDetection>>,

	#[sidecar(kind = "scene", extension_owned)]
	scene_tags: Option<Vec<SceneTag>>,

	#[sidecar(kind = "aesthetics", extension_owned)]
	quality_score: Option<f32>,

	/// Searchable tags (in core tag system)
	#[user_metadata]
	tags: Vec<Tag>,

	/// Custom fields in UserMetadata.custom_fields JSON
	#[custom_field]
	identified_people: Vec<PersonId>,

	#[custom_field]
	place_id: Option<PlaceId>,

	#[custom_field]
	moment_id: Option<MomentId>,

	/// Computed fields (derived, not stored)
	#[computed]
	has_faces: bool,

	#[computed]
	taken_at: Option<DateTime<Utc>>,
}

impl Photo {
	fn from_entry(entry: Entry) -> Self {
		Self {
			id: entry.id(),
			file: entry.clone(),
			exif: None,
			detected_faces: None,
			scene_tags: None,
			quality_score: None,
			tags: vec![],
			identified_people: vec![],
			place_id: None,
			moment_id: None,
			has_faces: false,
			taken_at: None,
		}
	}
}

#[derive(Serialize, Deserialize, Clone)]
#[model(version = "1.0.0")]
#[persist_strategy = "always"]
struct Person {
	id: PersonId,

	#[sync(shared, conflict = "last_writer_wins")]
	name: Option<String>,

	#[sync(shared)]
	thumbnail_photo_id: Option<Uuid>,

	#[sidecar(kind = "face_embeddings")]
	embeddings: Vec<Vec<f32>>,

	#[sync(device_owned)]
	photo_count: usize,

	#[vectorized(strategy = "average", model = "registered:face_embedding")]
	representative_embedding: Vec<f32>,
}

#[derive(Serialize, Deserialize, Clone)]
#[model(version = "1.0.0")]
#[persist_strategy = "always"]
struct Place {
	id: PlaceId,

	#[sync(shared)]
	name: String,

	#[sync(shared)]
	latitude: f64,

	#[sync(shared)]
	longitude: f64,

	#[sync(shared)]
	radius_meters: f32,

	#[sync(device_owned)]
	photo_count: usize,

	#[sync(shared)]
	thumbnail_photo_id: Option<Uuid>,
}

#[derive(Serialize, Deserialize, Clone)]
#[model(version = "1.0.0")]
struct Album {
	id: Uuid,

	#[sync(shared, conflict = "last_writer_wins")]
	name: String,

	#[sync(shared, conflict = "union_merge")]
	photo_ids: Vec<Uuid>,

	#[sync(shared)]
	cover_photo_id: Option<Uuid>,

	#[sync(shared)]
	created_at: DateTime<Utc>,

	#[custom_field]
	album_type: AlbumType,
}

#[derive(Serialize, Deserialize, Clone)]
#[model(version = "1.0.0")]
struct Moment {
	id: MomentId,

	#[sync(shared)]
	title: String,

	#[sync(shared)]
	start_date: DateTime<Utc>,

	#[sync(shared)]
	end_date: DateTime<Utc>,

	#[sync(shared)]
	location: Option<PlaceId>,

	#[sync(shared, conflict = "union_merge")]
	photo_ids: Vec<Uuid>,

	#[computed]
	photo_count: usize,
}

//==============================================================================
// Supporting Types
//==============================================================================

type PersonId = Uuid;
type PlaceId = Uuid;
type MomentId = Uuid;

#[derive(Serialize, Deserialize, Clone)]
struct FaceDetection {
	bbox: BoundingBox,
	confidence: f32,
	embedding: Vec<f32>, // 512-dim face embedding
	identified_as: Option<PersonId>,
}

#[derive(Serialize, Deserialize, Clone)]
struct BoundingBox {
	x: f32,
	y: f32,
	width: f32,
	height: f32,
}

#[derive(Serialize, Deserialize, Clone)]
struct SceneTag {
	label: String, // "beach", "sunset", "food", etc.
	confidence: f32,
}

#[derive(Serialize, Deserialize, Clone)]
enum AlbumType {
	Manual,    // User-created
	Smart,     // Auto-generated based on rules
	Shared,    // Shared with other users
	Favorites, // System album
	Hidden,    // System album
}

#[derive(Serialize, Deserialize, Clone)]
struct ExifData {
	camera_make: Option<String>,
	camera_model: Option<String>,
	lens_model: Option<String>,
	focal_length: Option<f32>,
	aperture: Option<f32>,
	iso: Option<u32>,
	shutter_speed: Option<String>,
	taken_at: Option<DateTime<Utc>>,
	gps: Option<GpsCoordinates>,
	orientation: Option<u8>,
}

#[derive(Serialize, Deserialize, Clone)]
struct GpsCoordinates {
	latitude: f64,
	longitude: f64,
	altitude: Option<f64>,
}

//==============================================================================
// Agent Memory
//==============================================================================

/// The Photos agent's "mind" - manages face clusters, place recognition, and moments
#[agent_memory]
#[memory_config(
    decay_rate = 0.01,  // Face clusters decay slowly
    summarization_trigger = 500  // Summarize after 500 photos
)]
struct PhotosMind {
	/// Event log of photo analysis
	/// Stored in: .sdlibrary/sidecars/extension/photos/memory/history.db
	history: TemporalMemory<PhotoEvent>,

	/// Knowledge graph of faces, places, relationships
	/// Stored in: .sdlibrary/sidecars/extension/photos/memory/knowledge.vss
	knowledge: AssociativeMemory<PhotoKnowledge>,

	/// Current analysis state
	/// Stored in: .sdlibrary/sidecars/extension/photos/memory/plan.json
	plan: WorkingMemory<AnalysisPlan>,
}

#[derive(Serialize, Deserialize, Clone)]
enum PhotoEvent {
	PhotoAnalyzed {
		photo_id: Uuid,
		faces_detected: usize,
		scene_tags: Vec<String>,
		location: Option<GpsCoordinates>,
	},
	PersonIdentified {
		person_id: PersonId,
		photo_id: Uuid,
		confidence: f32,
	},
	MomentCreated {
		moment_id: MomentId,
		photo_count: usize,
		date_range: (DateTime<Utc>, DateTime<Utc>),
	},
}

impl MemoryVariant for PhotoEvent {
	fn variant_name(&self) -> &'static str {
		match self {
			PhotoEvent::PhotoAnalyzed { .. } => "PhotoAnalyzed",
			PhotoEvent::PersonIdentified { .. } => "PersonIdentified",
			PhotoEvent::MomentCreated { .. } => "MomentCreated",
		}
	}
}

#[derive(Serialize, Deserialize, Clone)]
enum PhotoKnowledge {
	FaceCluster {
		person_id: PersonId,
		representative_embedding: Vec<f32>,
		photo_ids: Vec<Uuid>,
	},
	PlaceCluster {
		place_id: PlaceId,
		center: GpsCoordinates,
		photos: Vec<Uuid>,
	},
	ScenePattern {
		scene_type: String,
		typical_times: Vec<u8>,
		common_locations: Vec<PlaceId>,
	},
}

impl MemoryVariant for PhotoKnowledge {
	fn variant_name(&self) -> &'static str {
		match self {
			PhotoKnowledge::FaceCluster { .. } => "FaceCluster",
			PhotoKnowledge::PlaceCluster { .. } => "PlaceCluster",
			PhotoKnowledge::ScenePattern { .. } => "ScenePattern",
		}
	}
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct AnalysisPlan {
	pending_locations: Vec<SdPath>,
	photos_needing_faces: Vec<Uuid>,
	photos_needing_clustering: Vec<Uuid>,
	moments_to_generate: Vec<DateRange>,
}

#[derive(Serialize, Deserialize, Default)]
struct AnalyzePhotosState {
	photo_ids: Vec<Uuid>,
	current_index: usize,
}

type DateRange = (DateTime<Utc>, DateTime<Utc>);

/// Custom memory query methods
impl PhotosMind {
	/// Find photos with a specific person
	async fn photos_of_person(&self, person_id: PersonId) -> Vec<Uuid> {
		self.knowledge
			.query()
			.where_variant(PhotoKnowledge::FaceCluster)
			.where_field("person_id", equals(person_id))
			.collect()
			.await
			.unwrap_or_default()
			.into_iter()
			.flat_map(|k| match k {
				PhotoKnowledge::FaceCluster { photo_ids, .. } => photo_ids,
				_ => vec![],
			})
			.collect()
	}

	/// Find photos at a specific place
	async fn photos_at_place(&self, place_id: PlaceId) -> Vec<Uuid> {
		self.knowledge
			.query()
			.where_variant(PhotoKnowledge::PlaceCluster)
			.where_field("place_id", equals(place_id))
			.top_k(1000)
			.collect()
			.await
			.unwrap_or_default()
			.into_iter()
			.flat_map(|k| match k {
				PhotoKnowledge::PlaceCluster { photos, .. } => photos,
				_ => vec![],
			})
			.collect()
	}

	/// Suggest similar scenes
	async fn similar_scenes(&self, scene_type: &str) -> Vec<String> {
		self.knowledge
			.query_similar(scene_type)
			.where_variant(PhotoKnowledge::ScenePattern)
			.min_similarity(0.8)
			.top_k(5)
			.collect()
			.await
			.unwrap_or_default()
			.into_iter()
			.filter_map(|k| match k {
				PhotoKnowledge::ScenePattern { scene_type, .. } => Some(scene_type),
				_ => None,
			})
			.collect()
	}
}

//==============================================================================
// Agent Implementation
//==============================================================================

#[agent]
#[agent_trail(level = "debug", rotation = "daily")]
impl Photos {
	/// Lifecycle: Initialize on extension enable
	#[on_startup]
	async fn initialize(ctx: &AgentContext<PhotosMind>) -> AgentResult<()> {
		tracing::info!("Photos extension initialized");

		// Check if face detection model is registered
		if !ctx.models().is_registered("face_detection:photos_v1") {
			ctx.trace("Face detection model not found - will register on first use");
		}

		Ok(())
	}

	/// Event: New photo added to indexed location
	#[on_event(EntryCreated)]
	#[filter = ".extension().is_image()"]
	async fn on_new_photo(entry: Entry, ctx: &AgentContext<PhotosMind>) -> AgentResult<()> {
		ctx.trace(format!("New photo detected: {}", entry.name()));

		// Check if this entry is in a user-scoped location
		if !ctx.in_granted_scope(&entry.path()) {
			ctx.trace("Photo not in granted scope - skipping");
			return Ok(());
		}

		// Add to analysis queue
		let mut memory = ctx.memory().write().await;
		memory
			.plan
			.update(|mut plan| {
				plan.photos_needing_faces.push(entry.id());
				Ok(plan)
			})
			.await?;

		// Trigger batch analysis if queue is large enough
		if memory.plan.read().await.photos_needing_faces.len() >= 50 {
			ctx.jobs()
				.dispatch(
					analyze_photos_batch,
					memory.plan.read().await.photos_needing_faces.clone(),
				)
				.priority(Priority::Low)
				.when_idle()
				.await?;

			// Clear queue
			memory
				.plan
				.update(|mut plan| {
					plan.photos_needing_faces.clear();
					Ok(plan)
				})
				.await?;
		}

		Ok(())
	}

	/// Scheduled: Generate weekly memories
	#[scheduled(cron = "0 9 * * SUN")]
	async fn generate_weekly_memories(ctx: &AgentContext<PhotosMind>) -> AgentResult<()> {
		ctx.trace("Generating weekly memories");

		// Query photos from last week with location data
		let memory = ctx.memory().read().await;
		let last_week = memory
			.history
			.query()
			.where_variant(PhotoEvent::PhotoAnalyzed)
			.since(Duration::days(7))
			.where_field("location", is_not_null())
			.collect()
			.await?;

		if !last_week.is_empty() {
			ctx.jobs().dispatch(create_moments, last_week).await?;
		}

		Ok(())
	}
}

//==============================================================================
// Jobs - Face Detection
//==============================================================================

/// Analyze a batch of photos for faces
#[job(name = "analyze_photos_batch")]
fn analyze_photos_batch(ctx: &JobContext, state: &mut AnalyzePhotosState) -> Result<()> {
	ctx.progress(Progress::indeterminate("Analyzing photos for faces..."));

	let total = photo_ids.len();

	for (idx, photo_id) in photo_ids.into_iter().enumerate() {
		let photo = ctx.vdfs().get_entry(photo_id).await?;

		// Skip if already analyzed
		if ctx.sidecar_exists(photo.content_uuid(), "faces")? {
			continue;
		}

		// Run face detection task
		let faces = ctx.run(detect_faces_in_photo, (&photo,)).await?;

		// Save to extension sidecar
		ctx.save_sidecar(
			photo.content_uuid(),
			"faces",
			extension_id = "photos",
			&faces,
		)
		.await?;

		ctx.check_interrupt().await?;

		ctx.progress(Progress::simple(
			(idx + 1) as f32 / total as f32,
			format!("Analyzed {}/{} photos", idx + 1, total),
		));
	}

	// Cluster faces into people
	ctx.run(cluster_faces_into_people, (photo_ids,)).await?;

	// Generate tags
	ctx.run(generate_face_tags, (photo_ids,)).await?;

	ctx.progress(Progress::complete("Face analysis complete"));
	Ok(())
}

#[task(retries = 2, timeout_ms = 30000, requires_capability = "gpu_optional")]
async fn detect_faces_in_photo(ctx: &TaskContext, photo: &Entry) -> TaskResult<Vec<FaceDetection>> {
	// Load the image
	let image_bytes = photo.read().await?;

	// Run face detection model
	let detections = ctx
		.ai()
		.from_registered("face_detection:photos_v1")
		.detect_faces(&image_bytes)
		.await?;

	Ok(detections)
}

#[task(retries = 1, timeout_ms = 60000)]
async fn cluster_faces_into_people(ctx: &TaskContext, photo_ids: &[Uuid]) -> TaskResult<()> {
	// Read all face sidecars
	let mut all_faces: Vec<(Uuid, FaceDetection)> = Vec::new();

	for photo_id in photo_ids {
		let photo = ctx.vdfs().get_entry(*photo_id).await?;
		if let Ok(faces) = ctx
			.read_sidecar::<Vec<FaceDetection>>(photo.content_uuid(), "faces")
			.await
		{
			for face in faces {
				all_faces.push((*photo_id, face));
			}
		}
	}

	// Cluster faces by embedding similarity
	let clusters = dbscan_clustering(&all_faces, ctx.config().face_clustering_threshold);

	// Create or update Person models
	for cluster in clusters {
		let person_id = find_or_create_person(ctx, &cluster).await?;

		// Update photos with person ID
		for (photo_id, _) in cluster.faces {
			ctx.vdfs()
				.update_custom_field(photo_id, "identified_people", person_id)
				.await?;
		}
	}

	Ok(())
}

#[task]
async fn generate_face_tags(ctx: &TaskContext, photo_ids: &[Uuid]) -> TaskResult<()> {
	for photo_id in photo_ids {
		let photo = ctx.vdfs().get_entry(*photo_id).await?;

		// Read identified people from custom fields
		if let Ok(people) = photo.custom_field::<Vec<PersonId>>("identified_people") {
			for person_id in people {
				// Get person name
				if let Ok(person) = ctx.vdfs().get_model::<Person>(person_id).await {
					if let Some(name) = person.name {
						ctx.vdfs()
							.add_tag(photo.metadata_id(), &format!("#person:{}", name))
							.await?;
					}
				}
			}
		}
	}

	Ok(())
}

//==============================================================================
// Jobs - Place Identification
//==============================================================================

#[job(trigger = "user_initiated")]
async fn identify_places_in_location(ctx: &JobContext, location: SdPath) -> JobResult<()> {
	ctx.progress(Progress::indeterminate("Finding photos with GPS..."));

	// Get all photos with EXIF GPS data
	let photos = ctx
		.vdfs()
		.query_entries()
		.in_location(location)
		.of_type::<Image>()
		.where_metadata("exif.gps", is_not_null())
		.collect()
		.await?;

	// Group photos by geographic proximity
	let place_clusters = cluster_by_location(&photos, radius_meters = 500.0);

	for cluster in place_clusters {
		// Create or find place
		let place = find_or_create_place(ctx, &cluster).await?;

		// Reverse geocode to get place name
		if place.name == "Unknown Location" {
			let name = ctx.run(reverse_geocode, (&cluster.center,)).await?;
			ctx.vdfs()
				.update_model(place.id, |mut p| {
					p.name = name;
					Ok(p)
				})
				.await?;
		}

		// Tag photos with place
		for photo in &cluster.photos {
			ctx.vdfs()
				.update_custom_field(photo.id(), "place_id", place.id)
				.await?;

			ctx.vdfs()
				.add_tag(photo.metadata_id(), &format!("#place:{}", place.name))
				.await?;
		}
	}

	ctx.progress(Progress::complete("Places identified"));
	Ok(())
}

#[task]
async fn reverse_geocode(ctx: &TaskContext, coords: &GpsCoordinates) -> TaskResult<String> {
	// Use AI model for place name extraction
	#[derive(Serialize)]
	struct GeoPrompt {
		lat: f64,
		lon: f64,
	}

	let place_name = ctx
		.ai()
		.from_registered("llm:local")
		.prompt_template("identify_place.jinja")
		.render_with(&GeoPrompt {
			lat: coords.latitude,
			lon: coords.longitude,
		})?
		.generate_text()
		.await?;

	Ok(place_name)
}

//==============================================================================
// Jobs - Scene Understanding
//==============================================================================

#[job]
async fn analyze_scenes(ctx: &JobContext, photo_ids: Vec<Uuid>) -> JobResult<()> {
	for photo_id in photo_ids {
		let photo = ctx.vdfs().get_entry(photo_id).await?;

		// Run scene classification
		let scenes = ctx.run(classify_scene, (&photo,)).await?;

		// Save to sidecar
		ctx.save_sidecar(
			photo.content_uuid(),
			"scene",
			extension_id = "photos",
			&scenes,
		)
		.await?;

		// Generate tags for high-confidence scenes
		for scene in scenes {
			if scene.confidence > ctx.config().scene_confidence_threshold {
				ctx.vdfs()
					.add_tag(photo.metadata_id(), &format!("#scene:{}", scene.label))
					.await?;
			}
		}
	}

	Ok(())
}

#[task(requires_capability = "gpu_optional")]
async fn classify_scene(ctx: &TaskContext, photo: &Entry) -> TaskResult<Vec<SceneTag>> {
	let image_bytes = photo.read().await?;

	let classifications = ctx
		.ai()
		.from_registered("scene_classification:resnet50")
		.classify(&image_bytes)
		.await?;

	Ok(classifications)
}

//==============================================================================
// Jobs - Moment Generation
//==============================================================================

#[job]
async fn create_moments(ctx: &JobContext, photo_events: Vec<PhotoEvent>) -> JobResult<()> {
	// Group photos by time and location proximity
	let moment_groups = cluster_into_moments(&photo_events);

	for group in moment_groups {
		// Create moment
		let moment = Moment {
			id: Uuid::new_v4(),
			title: generate_moment_title(ctx, &group).await?,
			start_date: group.start_date,
			end_date: group.end_date,
			location: group.place_id,
			photo_ids: group.photo_ids.clone(),
			photo_count: group.photo_ids.len(),
		};

		// Save to VDFS
		ctx.vdfs().create_model(moment).await?;

		// Store in agent memory
		ctx.memory()
			.write()
			.await
			.history
			.append(PhotoEvent::MomentCreated {
				moment_id: moment.id,
				photo_count: moment.photo_count,
				date_range: (moment.start_date, moment.end_date),
			})
			.await?;
	}

	Ok(())
}

//==============================================================================
// Actions - User Operations
//==============================================================================

#[action]
async fn create_album(
	ctx: &ActionContext,
	name: String,
	photo_ids: Vec<Uuid>,
) -> ActionResult<ActionPreview> {
	Ok(ActionPreview {
		title: "Create Album",
		description: format!("Create album '{}' with {} photos", name, photo_ids.len()),
		changes: vec![Change::CreateModel {
			model_type: "Album",
			data: serde_json::to_value(&Album {
				id: Uuid::new_v4(),
				name: name.clone(),
				photo_ids: photo_ids.clone(),
				cover_photo_id: photo_ids.first().cloned(),
				created_at: Utc::now(),
				album_type: AlbumType::Manual,
			})?,
		}],
		reversible: true,
	})
}

#[action_execute]
async fn create_album_execute(
	ctx: &ActionContext,
	preview: ActionPreview,
) -> ActionResult<ExecutionResult> {
	for change in preview.changes {
		match change {
			Change::CreateModel { model_type, data } => {
				let album: Album = serde_json::from_value(data)?;
				ctx.vdfs().create_model(album).await?;
			}
			_ => {}
		}
	}

	Ok(ExecutionResult {
		success: true,
		message: "Album created successfully".to_string(),
	})
}

#[action]
async fn identify_person(
	ctx: &ActionContext,
	face_detections: Vec<(Uuid, FaceDetection)>,
	name: String,
) -> ActionResult<ActionPreview> {
	let photo_count = face_detections.len();

	Ok(ActionPreview {
		title: "Identify Person",
		description: format!("Identify {} photos as {}", photo_count, name),
		changes: face_detections
			.iter()
			.map(|(photo_id, face)| Change::UpdateCustomField {
				entry_id: *photo_id,
				field: "identified_people".to_string(),
				value: serde_json::to_value(&name).unwrap(),
			})
			.collect(),
		reversible: true,
	})
}

#[action]
async fn remove_photo_from_album(
	ctx: &ActionContext,
	album_id: Uuid,
	photo_id: Uuid,
) -> ActionResult<ActionPreview> {
	let album = ctx.vdfs().get_model::<Album>(album_id).await?;

	Ok(ActionPreview {
		title: "Remove from Album",
		description: format!("Remove photo from '{}'", album.name),
		changes: vec![Change::UpdateModel {
			model_id: album_id,
			field: "photo_ids",
			operation: "remove",
			value: serde_json::to_value(&photo_id)?,
		}],
		reversible: true,
	})
}

//==============================================================================
// Queries - User Searches
//==============================================================================

#[query("photos of {person_name}")]
async fn search_person(
	ctx: &QueryContext<PhotosMind>,
	person_name: String,
) -> QueryResult<Vec<Photo>> {
	// Find person by name
	let person = ctx
		.vdfs()
		.query_models::<Person>()
		.where_field("name", equals(&person_name))
		.first()
		.await?
		.ok_or(QueryError::NotFound)?;

	// Query agent memory for photos of this person
	let photo_ids = ctx.memory().read().await.photos_of_person(person.id).await;

	// Load Photo models
	let mut photos = Vec::new();
	for photo_id in photo_ids {
		if let Ok(photo) = ctx.vdfs().get_model::<Photo>(photo_id).await {
			photos.push(photo);
		}
	}

	Ok(photos)
}

#[query("photos from {place_name}")]
async fn search_place(
	ctx: &QueryContext<PhotosMind>,
	place_name: String,
) -> QueryResult<Vec<Photo>> {
	// Semantic search for place
	let place = ctx
		.vdfs()
		.query_models::<Place>()
		.search_semantic("name", similar_to(&place_name))
		.first()
		.await?
		.ok_or(QueryError::NotFound)?;

	// Get photos at this place from memory
	let photo_ids = ctx.memory().read().await.photos_at_place(place.id).await;

	let mut photos = Vec::new();
	for photo_id in photo_ids {
		if let Ok(photo) = ctx.vdfs().get_model::<Photo>(photo_id).await {
			photos.push(photo);
		}
	}

	Ok(photos)
}

#[query("photos with {scene_type}")]
async fn search_scene(
	ctx: &QueryContext<PhotosMind>,
	scene_type: String,
) -> QueryResult<Vec<Photo>> {
	// Use core tag system
	ctx.vdfs()
		.query_entries()
		.with_tag(&format!("#scene:{}", scene_type))
		.of_type::<Image>()
		.map(|entry| Photo::from_entry(entry))
		.collect()
		.await
}

//==============================================================================
// Helper Functions
//==============================================================================

fn dbscan_clustering(faces: &[(Uuid, FaceDetection)], threshold: f32) -> Vec<FaceCluster> {
	// DBSCAN clustering algorithm on face embeddings
	// Groups similar faces into clusters (potential same person)
	todo!("Implement DBSCAN clustering")
}

fn cluster_by_location(photos: &[Entry], radius_meters: f32) -> Vec<PlaceCluster> {
	// Geographic clustering using DBSCAN on GPS coordinates
	todo!("Implement geographic clustering")
}

fn cluster_into_moments(events: &[PhotoEvent]) -> Vec<MomentGroup> {
	// Temporal + spatial clustering for "moments"
	// Groups photos taken within 3 hours and 500m of each other
	todo!("Implement moment clustering")
}

async fn find_or_create_person(ctx: &TaskContext, cluster: &FaceCluster) -> TaskResult<PersonId> {
	// Find existing person or create new
	todo!("Implement person matching")
}

async fn find_or_create_place(ctx: &JobContext, cluster: &PlaceCluster) -> JobResult<Place> {
	// Find existing place or create new
	todo!("Implement place matching")
}

async fn generate_moment_title(ctx: &JobContext, group: &MomentGroup) -> JobResult<String> {
	// Use AI to generate title from scene tags and location
	#[derive(Serialize)]
	struct MomentPrompt {
		location: Option<String>,
		scenes: Vec<String>,
		date: String,
	}

	let title = ctx
		.ai()
		.from_registered("llm:local")
		.prompt_template("generate_moment_title.jinja")
		.render_with(&MomentPrompt {
			location: group.place_name.clone(),
			scenes: group.common_scenes.clone(),
			date: group.start_date.format("%B %Y").to_string(),
		})?
		.generate_text()
		.await?;

	Ok(title)
}

struct FaceCluster {
	faces: Vec<(Uuid, FaceDetection)>,
	centroid_embedding: Vec<f32>,
}

struct PlaceCluster {
	photos: Vec<Entry>,
	center: GpsCoordinates,
}

struct MomentGroup {
	photo_ids: Vec<Uuid>,
	start_date: DateTime<Utc>,
	end_date: DateTime<Utc>,
	place_id: Option<PlaceId>,
	place_name: Option<String>,
	common_scenes: Vec<String>,
}
