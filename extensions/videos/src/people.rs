//! The people roster: face clusters, and the names and corrections a
//! person made to them.
//!
//! Clustering is generation and runs again whenever faces change. The edit
//! log is the assertion half, so it is never derived from the clusters:
//! every pass seeds from the people that already exist, replays the log on
//! top, and writes the result. A named person keeps its id, and so its name,
//! for as long as faces keep matching it.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use spacedrive_sdk::job;
use spacedrive_sdk::prelude::*;

use crate::analyze::tag_segment;
use crate::models::{FaceRef, FacesDoc, Footage, Person, RosterEdit};

/// Cosine similarity at or above which a face joins a person. Tuned for
/// 512-d L2-normalized face embeddings.
const MATCH_THRESHOLD: f32 = 0.45;
/// Detections below this confidence are kept in the sidecar but not
/// clustered, because a blurry false positive would seed a ghost person.
const MIN_FACE_CONFIDENCE: f32 = 0.6;
/// Namespace for the id of a person first seen in a face, so an unnamed
/// cluster keeps its id when nothing about its faces changed.
const PERSON_NAMESPACE: Uuid = Uuid::from_u128(0x5d1b_7c2e_9a0f_4e61_8b3d_2f6a_c4e8_1907);

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ClusterState {
	pub people: usize,
}

/// Re-cluster every face the analyze job found.
#[job(name = "cluster_people")]
pub async fn cluster_people(ctx: &JobContext, state: &mut ClusterState) -> Result<()> {
	state.people = rebuild(ctx).await?;
	Ok(())
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EditState {
	/// `name`, `hide`, `show`, `merge`, `assign` or `exclude`.
	pub action: String,
	pub person: Option<Uuid>,
	pub name: Option<String>,
	pub into: Option<Uuid>,
	pub face: Option<FaceRef>,
}

/// Record one correction from the client and apply it.
///
/// The client runs this through `extensions.run_job`: naming a person,
/// hiding one, merging two, or pinning a face to a person or taking it off.
#[job(name = "edit_roster")]
pub async fn edit_roster(ctx: &JobContext, state: &mut EditState) -> Result<()> {
	let person = state
		.person
		.ok_or_else(|| Error::InvalidInput("a roster edit names a person".into()))?;
	match state.action.as_str() {
		"name" | "hide" | "show" => {}
		"merge" if state.into.is_some_and(|into| into != person) => {}
		"assign" | "exclude" if state.face.is_some() => {}
		other => {
			return Err(Error::InvalidInput(format!(
				"roster edit {other:?} is unknown or missing its target"
			)))
		}
	}
	let name = state
		.name
		.as_deref()
		.map(str::trim)
		.filter(|n| !n.is_empty())
		.map(str::to_string);
	let faces = if state.action == "merge" {
		ctx.vdfs()
			.query_models::<Person>()
			.collect()
			.await?
			.into_iter()
			.find(|p| p.id == person)
			.map(|p| p.faces)
			.unwrap_or_default()
	} else {
		Vec::new()
	};
	let stamp = spacedrive_sdk::clock::stamp()?;
	ctx.vdfs()
		.create_model(RosterEdit {
			id: Uuid::new_v4(),
			hlc: stamp.hlc,
			device_uuid: stamp.device_uuid,
			at_ms: spacedrive_sdk::clock::now_ms(),
			action: state.action.clone(),
			person,
			name,
			into: state.into,
			face: state.face.clone(),
			faces,
		})
		.await?;
	rebuild(ctx).await?;
	Ok(())
}

/// The edit log folded into what clustering has to honor.
#[derive(Default)]
struct Corrections {
	merged_into: HashMap<Uuid, Uuid>,
	names: HashMap<Uuid, Option<String>>,
	hidden: HashMap<Uuid, bool>,
	pinned: HashMap<FaceRef, Uuid>,
	excluded: HashSet<(FaceRef, Uuid)>,
}

impl Corrections {
	fn from_log(mut edits: Vec<RosterEdit>) -> Self {
		edits.sort_by(|a, b| {
			(&a.hlc, a.at_ms, a.device_uuid, a.id).cmp(&(&b.hlc, b.at_ms, b.device_uuid, b.id))
		});
		let mut c = Self::default();
		for edit in edits {
			match (edit.action.as_str(), edit.into, edit.face) {
				("name", _, _) => {
					c.names.insert(edit.person, edit.name);
				}
				("hide", _, _) => {
					c.hidden.insert(edit.person, true);
				}
				("show", _, _) => {
					c.hidden.insert(edit.person, false);
				}
				("merge", Some(into), _) => {
					c.merged_into.insert(edit.person, into);
					for face in edit.faces {
						c.excluded.remove(&(face.clone(), into));
						c.pinned.insert(face, into);
					}
				}
				("assign", _, Some(face)) => {
					c.excluded.remove(&(face.clone(), edit.person));
					c.pinned.insert(face, edit.person);
				}
				("exclude", _, Some(face)) => {
					if c.pinned.get(&face) == Some(&edit.person) {
						c.pinned.remove(&face);
					}
					c.excluded.insert((face, edit.person));
				}
				_ => {}
			}
		}
		c
	}

	/// The person a merge chain ends at. A cycle stops where it repeats.
	fn canonical(&self, mut person: Uuid) -> Uuid {
		let mut seen = HashSet::new();
		while let Some(&into) = self.merged_into.get(&person) {
			if !seen.insert(person) {
				break;
			}
			person = into;
		}
		person
	}

	/// A name or hide that lands on a merged person follows it, newest
	/// owner first.
	fn name_of(&self, person: Uuid) -> Option<String> {
		self.names
			.iter()
			.filter(|(p, _)| self.canonical(**p) == person)
			.find_map(|(p, n)| if *p == person { n.clone() } else { None })
			.or_else(|| {
				self.names
					.iter()
					.filter(|(p, _)| self.canonical(**p) == person)
					.find_map(|(_, n)| n.clone())
			})
	}
}

/// The person tag a pass leaves on each file: `old` comes off every file
/// that should not carry it, `new` goes on the files the person is in.
struct Retag {
	old: Option<String>,
	new: Option<String>,
	files: HashSet<Uuid>,
}

struct Cluster {
	id: Uuid,
	seed: Vec<f32>,
	sum: Vec<f32>,
	faces: Vec<FaceRef>,
}

impl Cluster {
	fn centroid(&self) -> Vec<f32> {
		if self.faces.is_empty() {
			self.seed.clone()
		} else {
			normalized(&self.sum)
		}
	}

	fn add(&mut self, face: FaceRef, embedding: &[f32]) {
		if self.sum.len() != embedding.len() {
			self.sum = vec![0.0; embedding.len()];
		}
		for (s, e) in self.sum.iter_mut().zip(embedding) {
			*s += e;
		}
		self.faces.push(face);
	}
}

fn normalized(v: &[f32]) -> Vec<f32> {
	let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
	if norm == 0.0 {
		return v.to_vec();
	}
	v.iter().map(|x| x / norm).collect()
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
	if a.len() != b.len() || a.is_empty() {
		return -1.0;
	}
	a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Cluster every face, replay the edit log, and write the roster, each
/// video's per-shot people, and a `Videos/People/<name>` tag on every file
/// a named person appears in. Returns how many people have faces.
pub async fn rebuild(ctx: &JobContext) -> Result<usize> {
	let mut footage = ctx.vdfs().query_models::<Footage>().collect().await?;
	footage.sort_by_key(|f| f.id);
	let existing = ctx.vdfs().query_models::<Person>().collect().await?;
	let corrections =
		Corrections::from_log(ctx.vdfs().query_models::<RosterEdit>().collect().await?);

	let mut faces: Vec<(FaceRef, Vec<f32>)> = Vec::new();
	for file in &footage {
		if file.status != "analyzed" || !ctx.sidecar_exists(file.id, "faces")? {
			continue;
		}
		let doc: FacesDoc = ctx.read_sidecar(file.id, "faces").await?;
		for shot in &doc.shots {
			for (index, face) in shot.faces.iter().enumerate() {
				if face.embedding.is_empty() || face.confidence < MIN_FACE_CONFIDENCE {
					continue;
				}
				faces.push((
					FaceRef {
						media_uuid: file.id,
						shot_id: shot.shot_id.clone(),
						index: index as u32,
					},
					normalized(&face.embedding),
				));
			}
		}
	}

	let mut clusters: Vec<Cluster> = Vec::new();
	for person in &existing {
		if corrections.canonical(person.id) == person.id && !person.centroid.is_empty() {
			clusters.push(Cluster {
				id: person.id,
				seed: person.centroid.clone(),
				sum: Vec::new(),
				faces: Vec::new(),
			});
		}
	}
	let cluster_for = |clusters: &mut Vec<Cluster>, id: Uuid, seed: &[f32]| -> usize {
		match clusters.iter().position(|c| c.id == id) {
			Some(i) => i,
			None => {
				clusters.push(Cluster {
					id,
					seed: seed.to_vec(),
					sum: Vec::new(),
					faces: Vec::new(),
				});
				clusters.len() - 1
			}
		}
	};

	let (pinned, free): (Vec<_>, Vec<_>) = faces
		.into_iter()
		.partition(|(face, _)| corrections.pinned.contains_key(face));
	for (face, embedding) in pinned {
		let id = corrections.canonical(corrections.pinned[&face]);
		let i = cluster_for(&mut clusters, id, &embedding);
		clusters[i].add(face, &embedding);
	}
	for (face, embedding) in free {
		let best = clusters
			.iter()
			.enumerate()
			.filter(|(_, c)| !corrections.excluded.contains(&(face.clone(), c.id)))
			.map(|(i, c)| (i, cosine(&c.centroid(), &embedding)))
			.max_by(|a, b| a.1.total_cmp(&b.1));
		match best {
			Some((i, similarity)) if similarity >= MATCH_THRESHOLD => {
				clusters[i].add(face, &embedding)
			}
			_ => {
				// A face taken off the person it founded must not found that
				// person again, so a taken id moves to the next generation.
				let id = (0u32..)
					.map(|generation| {
						Uuid::new_v5(
							&PERSON_NAMESPACE,
							format!(
								"{}/{}/{}/{generation}",
								face.media_uuid, face.shot_id, face.index
							)
							.as_bytes(),
						)
					})
					.find(|id| {
						!clusters.iter().any(|c| c.id == *id)
							&& corrections.canonical(*id) == *id
							&& !corrections.excluded.contains(&(face.clone(), *id))
					})
					.expect("an unbounded range finds a free id");
				let i = cluster_for(&mut clusters, id, &embedding);
				clusters[i].add(face, &embedding);
			}
		}
	}

	let mut person_of: HashMap<FaceRef, Uuid> = HashMap::new();
	let mut written = HashSet::new();
	let mut with_faces = 0;
	let mut retags: Vec<Retag> = Vec::new();
	for cluster in &clusters {
		let previous = existing.iter().find(|p| p.id == cluster.id);
		let name = corrections.name_of(cluster.id);
		let hidden = corrections
			.hidden
			.get(&cluster.id)
			.copied()
			.unwrap_or(false);
		let files: HashSet<Uuid> = cluster.faces.iter().map(|f| f.media_uuid).collect();
		let shots: HashSet<(Uuid, &str)> = cluster
			.faces
			.iter()
			.map(|f| (f.media_uuid, f.shot_id.as_str()))
			.collect();
		for face in &cluster.faces {
			person_of.insert(face.clone(), cluster.id);
		}
		if !cluster.faces.is_empty() {
			with_faces += 1;
		}
		let tag = name
			.as_deref()
			.filter(|_| !hidden && !cluster.faces.is_empty())
			.map(|n| format!("Videos/People/{}", tag_segment(n)));
		retags.push(Retag {
			old: previous.and_then(|p| p.tag.clone()),
			new: tag.clone(),
			files: files.clone(),
		});
		ctx.vdfs()
			.create_model(Person {
				id: cluster.id,
				name,
				hidden,
				cover: cluster.faces.first().cloned(),
				centroid: cluster.centroid(),
				shot_count: shots.len() as u32,
				file_count: files.len() as u32,
				faces: cluster.faces.clone(),
				tag,
			})
			.await?;
		written.insert(cluster.id);
	}
	// A person merged away or left with no faces is emptied rather than
	// deleted: the store has no delete, and an empty row lists nothing.
	for person in &existing {
		if written.contains(&person.id) {
			continue;
		}
		retags.push(Retag {
			old: person.tag.clone(),
			new: None,
			files: HashSet::new(),
		});
		ctx.vdfs()
			.create_model(Person {
				name: None,
				faces: Vec::new(),
				cover: None,
				centroid: Vec::new(),
				shot_count: 0,
				file_count: 0,
				tag: None,
				..person.clone()
			})
			.await?;
	}

	for file in &mut footage {
		if file.status != "analyzed" {
			continue;
		}
		let mut changed = false;
		for shot in &mut file.shots {
			let mut people: Vec<Uuid> = person_of
				.iter()
				.filter(|(face, _)| face.media_uuid == file.id && face.shot_id == shot.id)
				.map(|(_, person)| *person)
				.collect();
			people.sort();
			people.dedup();
			if people != shot.people {
				shot.people = people;
				changed = true;
			}
		}
		if changed {
			ctx.vdfs()
				.create_model_for_content(file.id, file.clone())
				.await?;
		}
	}

	for Retag { old, new, files } in &retags {
		for file in &footage {
			let carries = files.contains(&file.id);
			if let Some(old) = old
				.as_deref()
				.filter(|old| Some(*old) != new.as_deref() || !carries)
			{
				ctx.vdfs().remove_tag(file.record, old).await?;
			}
			if let Some(tag) = new.as_deref().filter(|_| carries) {
				ctx.vdfs().add_tag(file.record, tag).await?;
			}
		}
	}

	ctx.log(&format!(
		"Roster: {with_faces} people from {} faces",
		person_of.len()
	));
	Ok(with_faces)
}
