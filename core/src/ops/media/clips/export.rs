//! `media.timeline.export`: a rough cut for an editor.

use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::Arc,
};

use sd_timeline::{Media, Rate, TimePoint, Timeline};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::footage::{display_name, local_path, probe, proxy_path, Footage, TimeInput};
use crate::{
	context::CoreContext,
	domain::SdPath,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
};

/// The most clips one timeline takes.
pub const MAX_EXPORT_CLIPS: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TimelineFormat {
	/// FCPXML 1.10, for Final Cut Pro and Resolve.
	Fcpxml,
	/// CMX 3600, for Resolve and Premiere Pro.
	Edl,
	/// OpenTimelineIO JSON.
	Otio,
}

impl TimelineFormat {
	pub const ALL: [Self; 3] = [Self::Fcpxml, Self::Edl, Self::Otio];

	pub fn extension(self) -> &'static str {
		match self {
			Self::Fcpxml => "fcpxml",
			Self::Edl => "edl",
			Self::Otio => "otio",
		}
	}
}

/// Which file each clip points the editor at.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ExportMedia {
	#[default]
	Original,
	/// The 720p proxy, which has the original's frames.
	Proxy,
}

/// One clip of the cut, in timeline order.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExportClip {
	/// A video on this device.
	pub path: SdPath,
	#[serde(rename = "in")]
	pub in_point: TimeInput,
	/// Exclusive.
	#[serde(rename = "out")]
	pub out_point: TimeInput,
	#[serde(default)]
	pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TimelineExportInput {
	/// The project and file name.
	pub name: String,
	pub clips: Vec<ExportClip>,
	/// Every format when empty.
	#[serde(default)]
	pub formats: Vec<TimelineFormat>,
	#[serde(default)]
	pub media: ExportMedia,
	/// A directory on this device to write the files into. A taken name gets
	/// a number. Without it, the documents come back in the output.
	#[serde(default)]
	pub destination: Option<SdPath>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct TimelineSummary {
	pub rate: Rate,
	pub duration: TimePoint,
	pub clips: u32,
}

/// A timeline file the export wrote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct WrittenTimeline {
	pub format: TimelineFormat,
	pub path: SdPath,
}

/// A timeline file's text, when no destination was given.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct TimelineDocument {
	pub format: TimelineFormat,
	/// The name to save it under.
	pub file_name: String,
	pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct TimelineExportOutput {
	pub timeline: TimelineSummary,
	pub files: Vec<WrittenTimeline>,
	pub documents: Vec<TimelineDocument>,
	/// What an editor will do differently from what the timeline says.
	pub warnings: Vec<String>,
}

pub struct TimelineExportAction {
	input: TimelineExportInput,
}

impl LibraryAction for TimelineExportAction {
	type Input = TimelineExportInput;
	type Output = TimelineExportOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		if file_stem(&input.name).is_empty() {
			return Err("name must hold at least one letter or digit".to_string());
		}
		if input.clips.is_empty() || input.clips.len() > MAX_EXPORT_CLIPS {
			return Err(format!("a timeline takes 1 to {MAX_EXPORT_CLIPS} clips"));
		}
		for clip in &input.clips {
			local_path(&clip.path)?;
		}
		if let Some(destination) = &input.destination {
			local_path(destination)?;
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let documents = render(&context, &library, &self.input)
			.await
			.map_err(ActionError::InvalidInput)?;
		let Some(destination) = &self.input.destination else {
			return Ok(documents);
		};
		let directory = local_path(destination).map_err(ActionError::InvalidInput)?;
		let mut output = documents;
		for document in std::mem::take(&mut output.documents) {
			let path = write_beside(&directory, &document.file_name, &document.content)
				.await
				.map_err(|error| {
					ActionError::Internal(format!(
						"could not write {}: {error}",
						document.file_name
					))
				})?;
			output.files.push(WrittenTimeline {
				format: document.format,
				path: SdPath::local(path),
			});
		}
		Ok(output)
	}

	fn action_kind(&self) -> &'static str {
		"media.timeline.export"
	}
}

/// Builds the timeline and writes it in each format asked for, as documents.
pub(crate) async fn render(
	context: &CoreContext,
	library: &Library,
	input: &TimelineExportInput,
) -> Result<TimelineExportOutput, String> {
	let mut media: Vec<Media> = Vec::new();
	let mut index_of: HashMap<PathBuf, usize> = HashMap::new();
	let mut clips = Vec::with_capacity(input.clips.len());
	for (position, clip) in input.clips.iter().enumerate() {
		let path = local_path(&clip.path)?;
		let index = match index_of.get(&path) {
			Some(index) => *index,
			None => {
				media.push(media_of(context, library, &path, input.media).await?);
				index_of.insert(path.clone(), media.len() - 1);
				media.len() - 1
			}
		};
		let rate = media[index].rate;
		let source_in = clip
			.in_point
			.frame_at(rate)
			.map_err(|reason| format!("clip {}: {reason}", position + 1))?;
		let source_out = clip
			.out_point
			.frame_at(rate)
			.map_err(|reason| format!("clip {}: {reason}", position + 1))?;
		clips.push(sd_timeline::Clip {
			media: index,
			source_in,
			source_out,
			label: clip.label.clone().filter(|label| !label.trim().is_empty()),
		});
	}
	let first = &media[clips[0].media];
	let timeline = Timeline {
		name: input.name.trim().to_string(),
		rate: first.rate,
		width: first.width,
		height: first.height,
		drop_frame: first.drop_frame,
		clips,
		media,
	};
	let duration = timeline.duration().map_err(|error| error.to_string())?;
	let formats: Vec<TimelineFormat> = if input.formats.is_empty() {
		TimelineFormat::ALL.to_vec()
	} else {
		input.formats.clone()
	};
	let stem = file_stem(&input.name);
	let mut documents = Vec::new();
	for format in formats {
		if documents
			.iter()
			.any(|d: &TimelineDocument| d.format == format)
		{
			continue;
		}
		let content = match format {
			TimelineFormat::Fcpxml => sd_timeline::fcpxml::write(&timeline),
			TimelineFormat::Edl => sd_timeline::edl::write(&timeline),
			TimelineFormat::Otio => sd_timeline::otio::write(&timeline),
		}
		.map_err(|error| error.to_string())?;
		documents.push(TimelineDocument {
			format,
			file_name: format!("{stem}.{}", format.extension()),
			content,
		});
	}
	Ok(TimelineExportOutput {
		timeline: TimelineSummary {
			rate: timeline.rate,
			duration: timeline.rate.point(duration, 0, timeline.drop_frame),
			clips: timeline.clips.len() as u32,
		},
		files: Vec::new(),
		documents,
		warnings: timeline.warnings(),
	})
}

/// The original's timing and the referenced file's picture and sound. A
/// proxy has the original's frames but not always its timecode track, so the
/// timecode is the original's whichever file is referenced.
async fn media_of(
	context: &CoreContext,
	library: &Library,
	path: &Path,
	kind: ExportMedia,
) -> Result<Media, String> {
	let original = probe(path).await?;
	let (referenced, picture) = match kind {
		ExportMedia::Original => (path.to_path_buf(), original.clone()),
		ExportMedia::Proxy => {
			let video = Footage::load(context, library, path).await?;
			let proxy = proxy_path(library, video.content_uuid);
			if !video.proxy {
				return Err(format!(
					"{} has no proxy yet; export with media: original, or generate proxies first",
					display_name(path)
				));
			}
			let picture = probe(&proxy).await?;
			(proxy, picture)
		}
	};
	Ok(Media {
		path: referenced,
		name: display_name(path),
		rate: original.rate,
		width: picture.width,
		height: picture.height,
		start_timecode: original.start_timecode,
		drop_frame: original.drop_frame,
		duration: original.duration,
		audio: picture.audio,
	})
}

/// A file name from a project name: path separators and characters editors
/// or file systems reject become `_`.
pub(crate) fn file_stem(name: &str) -> String {
	let stem: String = name
		.trim()
		.chars()
		.map(|c| match c {
			'/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
			c if c.is_control() => '_',
			c => c,
		})
		.take(120)
		.collect();
	let stem = stem
		.trim_matches(|c: char| c == '.' || c.is_whitespace())
		.to_string();
	if stem.chars().any(char::is_alphanumeric) {
		stem
	} else {
		String::new()
	}
}

/// Writes `content` as `name` in `directory`, or as `name (2)` and so on
/// when the name is taken. Never replaces a file.
async fn write_beside(directory: &Path, name: &str, content: &str) -> std::io::Result<PathBuf> {
	use tokio::io::AsyncWriteExt;
	tokio::fs::create_dir_all(directory).await?;
	let path = Path::new(name);
	let stem = path
		.file_stem()
		.map(|s| s.to_string_lossy().into_owned())
		.unwrap_or_default();
	let extension = path
		.extension()
		.map(|e| e.to_string_lossy().into_owned())
		.unwrap_or_default();
	for n in 1..1000 {
		let candidate = if n == 1 {
			directory.join(name)
		} else {
			directory.join(format!("{stem} ({n}).{extension}"))
		};
		match tokio::fs::OpenOptions::new()
			.write(true)
			.create_new(true)
			.open(&candidate)
			.await
		{
			Ok(mut file) => {
				file.write_all(content.as_bytes()).await?;
				file.sync_all().await?;
				return Ok(candidate);
			}
			Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
			Err(error) => return Err(error),
		}
	}
	Err(std::io::Error::other("every numbered name is taken"))
}

crate::register_library_action!(TimelineExportAction, "media.timeline.export");

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn file_stems_are_safe() {
		assert_eq!(file_stem("Train rough cut"), "Train rough cut");
		assert_eq!(file_stem("a/b:c"), "a_b_c");
		assert_eq!(file_stem("  ..hidden.  "), "hidden");
		assert_eq!(file_stem("///"), "");
	}

	#[tokio::test]
	async fn writes_never_replace_a_file() {
		let dir = tempfile::tempdir().unwrap();
		let first = write_beside(dir.path(), "cut.edl", "one").await.unwrap();
		let second = write_beside(dir.path(), "cut.edl", "two").await.unwrap();
		assert_eq!(first, dir.path().join("cut.edl"));
		assert_eq!(second, dir.path().join("cut (2).edl"));
		assert_eq!(std::fs::read_to_string(first).unwrap(), "one");
	}
}
