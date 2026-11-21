//! The models Spacedrive can download and run, one entry per model.
//!
//! Every entry pins its files by URL, size and SHA-256, and carries the
//! license the user accepts before download. Only models whose code and
//! weights both allow commercial use, and whose training data does too as far
//! as the model card says, belong here. The registry picks the first
//! installed entry of a category, so order is preference.

use super::Category;

/// Which loader runs a model's files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
	/// MediaPipe BlazeFace full-range, converted to ONNX.
	BlazeFace,
	/// ArcFace-style ResNet face embedding over 112x112 aligned crops.
	ArcFace,
	/// SigLIP image and text towers, zero-shot over [`SCENE_LABELS`].
	Siglip,
	/// E5 sentence embedding, mean pooled.
	E5,
}

#[derive(Debug)]
pub struct ModelFile {
	/// Name on disk, under the model's directory.
	pub name: &'static str,
	pub url: &'static str,
	pub sha256: &'static str,
	pub size: u64,
}

#[derive(Debug)]
pub struct ModelSpec {
	pub id: &'static str,
	pub name: &'static str,
	pub category: Category,
	pub engine: Engine,
	pub description: &'static str,
	/// SPDX identifier of the weights' license.
	pub license: &'static str,
	pub license_url: &'static str,
	pub files: &'static [ModelFile],
}

impl ModelSpec {
	pub fn size(&self) -> u64 {
		self.files.iter().map(|f| f.size).sum()
	}

	/// A file's id for the download job: `<model id>/<file name>`.
	pub fn file_id(&self, file: &ModelFile) -> String {
		format!("{}/{}", self.id, file.name)
	}
}

/// Empty until the model shortlist is approved; the registry answers every
/// category `not_available` meanwhile.
pub static CATALOG: &[ModelSpec] = &[];

pub fn find(id: &str) -> Option<&'static ModelSpec> {
	CATALOG.iter().find(|spec| spec.id == id)
}

pub fn for_category(category: Category) -> impl Iterator<Item = &'static ModelSpec> {
	CATALOG.iter().filter(move |spec| spec.category == category)
}

/// The catalog file a download job id names.
pub fn find_file(file_id: &str) -> Option<(&'static ModelSpec, &'static ModelFile)> {
	let (model, name) = file_id.split_once('/')?;
	let spec = find(model)?;
	spec.files
		.iter()
		.find(|f| f.name == name)
		.map(|file| (spec, file))
}

/// Scene labels Spacedrive classifies against. Zero-shot models score each
/// label's prompt, so the list is Spacedrive's, not the model's.
pub const SCENE_LABELS: &[&str] = &[
	"airport",
	"amusement park",
	"aquarium",
	"art gallery",
	"auditorium",
	"bakery",
	"bar",
	"bathroom",
	"beach",
	"bedroom",
	"bridge",
	"building facade",
	"campsite",
	"canyon",
	"castle",
	"cave",
	"church",
	"city street",
	"classroom",
	"cliff",
	"coast",
	"concert",
	"conference room",
	"construction site",
	"corridor",
	"countryside",
	"desert",
	"dining room",
	"farm",
	"field",
	"forest",
	"garden",
	"glacier",
	"gym",
	"harbor",
	"highway",
	"hospital",
	"hotel room",
	"ice rink",
	"kitchen",
	"lake",
	"library",
	"living room",
	"market",
	"mountain",
	"museum",
	"night sky",
	"office",
	"park",
	"parking lot",
	"playground",
	"pool",
	"restaurant",
	"river",
	"road",
	"rooftop",
	"ruins",
	"shop",
	"skyline",
	"snow",
	"stadium",
	"stage",
	"studio",
	"subway station",
	"sunset",
	"swamp",
	"temple",
	"train station",
	"underwater",
	"valley",
	"vineyard",
	"volcano",
	"waterfall",
	"wedding",
	"workshop",
];
