//! `media.footage.status`: what the footage work has already made for a file.

use std::{collections::HashSet, path::Path, sync::Arc};

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use super::{local_footage_path, VIDEOS_EXTENSION_ID};
use crate::{
	context::CoreContext,
	domain::{sidecar::content_uuid_of_file, SdPath, SidecarFormat, SidecarKind},
	infra::{
		api::SessionContext,
		query::{LibraryQuery, QueryError, QueryResult},
	},
	ops::{
		files::query::sidecars::{content_sidecars, CORE_PRODUCER},
		indexing::metadata::refuse_dataless,
	},
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FootageStatusInput {
	/// A video file on this device.
	pub path: SdPath,
}

/// The sidecars a video's bytes have. They follow content, so a copy or a
/// renamed file reports what was made for the same bytes elsewhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FootageStatusOutput {
	pub content_uuid: Uuid,
	pub transcript: bool,
	pub proxy: bool,
	/// Shots in the shot list, `None` until shot detection has run.
	pub shots: Option<u32>,
	/// Shots the Videos extension has analyzed, `None` until it has run on
	/// this file.
	pub analyzed_shots: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FootageStatusQuery {
	input: FootageStatusInput,
}

impl LibraryQuery for FootageStatusQuery {
	type Input = FootageStatusInput;
	type Output = FootageStatusOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or(QueryError::LibraryNotFound(library_id))?;
		let path = local_footage_path(&self.input.path).map_err(QueryError::InvalidInput)?;
		// Hashing a cloud placeholder would download it just to show status.
		refuse_dataless(&path)
			.await
			.map_err(|error| QueryError::InvalidInput(error.to_string()))?;
		let content_uuid = content_uuid_of_file(&path).await.map_err(|error| {
			QueryError::InvalidInput(format!("could not read {}: {error}", path.display()))
		})?;
		let library_path = library.path();
		let sidecars = content_sidecars(library_path, content_uuid)
			.await
			.map_err(|error| {
				QueryError::Internal(format!("could not list the content sidecars: {error}"))
			})?;
		let core = |kind: &str, format: &str| {
			sidecars.iter().any(|sidecar| {
				sidecar.producer == CORE_PRODUCER
					&& sidecar.kind == kind
					&& sidecar.format == format
			})
		};
		Ok(FootageStatusOutput {
			content_uuid,
			transcript: core("transcript", SidecarFormat::Json.extension()),
			proxy: core("proxy", SidecarFormat::Mp4.extension()),
			shots: shot_count(library_path, content_uuid).await?,
			analyzed_shots: analyzed_shots(library_path, content_uuid).await?,
		})
	}
}

crate::register_library_query!(FootageStatusQuery, "media.footage.status");

async fn shot_count(library_path: &Path, content_uuid: Uuid) -> QueryResult<Option<u32>> {
	let list = crate::ops::media::shots::read_shots(library_path, content_uuid)
		.await
		.map_err(|error| QueryError::Internal(format!("could not read the shot list: {error}")))?;
	Ok(list.map(|list| u32::try_from(list.shots.len()).unwrap_or(u32::MAX)))
}

/// How many shots the Videos extension has analyzed, or `None` before it
/// has. A run with only a face detector writes `faces` and no `scene`, so a
/// shot counts when either sidecar has it.
pub(super) async fn analyzed_shots(
	library_path: &Path,
	content_uuid: Uuid,
) -> QueryResult<Option<u32>> {
	let scene = read_analyzed(library_path, content_uuid, "scene").await?;
	let faces = read_analyzed(library_path, content_uuid, "faces").await?;
	if scene.is_none() && faces.is_none() {
		return Ok(None);
	}
	let shots: HashSet<String> = scene
		.into_iter()
		.chain(faces)
		.flatten()
		.map(|shot| shot.shot_id)
		.collect();
	Ok(Some(u32::try_from(shots.len()).unwrap_or(u32::MAX)))
}

/// One shot the Videos extension has written a description for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ShotDescription {
	pub shot_id: String,
	pub description: String,
}

/// The descriptions the Videos extension wrote for a video's shots, empty
/// until it has analyzed the video or when it found no caption model.
pub(crate) async fn shot_descriptions(
	library_path: &Path,
	content_uuid: Uuid,
) -> QueryResult<Vec<ShotDescription>> {
	let shots = read_analyzed(library_path, content_uuid, "scene")
		.await?
		.unwrap_or_default();
	Ok(shots
		.into_iter()
		.filter_map(|shot| {
			Some(ShotDescription {
				shot_id: shot.shot_id,
				description: shot.description?,
			})
		})
		.collect())
}

#[derive(Deserialize)]
struct AnalyzedShot {
	#[serde(default)]
	shot_id: String,
	#[serde(default)]
	description: Option<String>,
}

fn analysis_path(
	library_path: &Path,
	content_uuid: Uuid,
	kind: &str,
) -> QueryResult<std::path::PathBuf> {
	let kind = SidecarKind::extension(VIDEOS_EXTENSION_ID, kind).map_err(QueryError::Internal)?;
	Ok(library_path
		.join("sidecars")
		.join(sd_sidecar_path::relative_path(
			&content_uuid,
			&kind.directory(),
			"default",
			SidecarFormat::Json.extension(),
		)))
}

/// The Videos extension's `scene` and `faces` sidecars hold one entry per
/// shot it has analyzed. Their shape belongs to the extension, so only the
/// shot id and the description are read, and both are optional.
async fn read_analyzed(
	library_path: &Path,
	content_uuid: Uuid,
	kind: &str,
) -> QueryResult<Option<Vec<AnalyzedShot>>> {
	#[derive(Deserialize)]
	struct AnalysisDoc {
		#[serde(default)]
		shots: Vec<AnalyzedShot>,
	}

	let path = analysis_path(library_path, content_uuid, kind)?;
	let bytes = match tokio::fs::read(&path).await {
		Ok(bytes) => bytes,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
		Err(error) => {
			return Err(QueryError::Internal(format!(
				"could not read the {kind} sidecar: {error}"
			)))
		}
	};
	let doc: AnalysisDoc = serde_json::from_slice(&bytes).map_err(|error| {
		QueryError::Internal(format!("could not read the {kind} sidecar: {error}"))
	})?;
	Ok(Some(doc.shots))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn scene_and_faces_sidecars_yield_described_shots_and_the_analyzed_count() {
		let library = tempfile::tempdir().unwrap();
		let content_uuid = Uuid::new_v4();
		assert_eq!(
			analyzed_shots(library.path(), content_uuid).await.unwrap(),
			None
		);
		assert!(shot_descriptions(library.path(), content_uuid)
			.await
			.unwrap()
			.is_empty());

		let faces = analysis_path(library.path(), content_uuid, "faces").unwrap();
		tokio::fs::create_dir_all(faces.parent().unwrap())
			.await
			.unwrap();
		tokio::fs::write(
			&faces,
			r#"{"version":1,"shots":[{"shot_id":"b","faces":[]},{"shot_id":"c","faces":[]}]}"#,
		)
		.await
		.unwrap();
		assert_eq!(
			analyzed_shots(library.path(), content_uuid).await.unwrap(),
			Some(2)
		);

		let path = analysis_path(library.path(), content_uuid, "scene").unwrap();
		tokio::fs::create_dir_all(path.parent().unwrap())
			.await
			.unwrap();
		tokio::fs::write(
			&path,
			r#"{"version":1,"shots":[
				{"shot_id":"a","description":"A dog on a beach","places":[]},
				{"shot_id":"b","description":null,"places":[{"label":"beach","score":0.9}]}
			]}"#,
		)
		.await
		.unwrap();

		assert_eq!(
			analyzed_shots(library.path(), content_uuid).await.unwrap(),
			Some(3)
		);
		assert_eq!(
			shot_descriptions(library.path(), content_uuid)
				.await
				.unwrap(),
			vec![ShotDescription {
				shot_id: "a".to_string(),
				description: "A dog on a beach".to_string(),
			}]
		);
	}
}
