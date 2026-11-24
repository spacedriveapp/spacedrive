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
	/// Florence-2 vision encoder, BART encoder and decoder, greedy decoding.
	Florence2,
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

pub static CATALOG: &[ModelSpec] = &[
	ModelSpec {
		id: "blazeface-full-range",
		name: "BlazeFace full-range",
		category: Category::FaceDetection,
		engine: Engine::BlazeFace,
		description: "MediaPipe face detector for faces up to about 5 m from the camera. 1 MB.",
		license: "Apache-2.0",
		license_url: "https://huggingface.co/litert-community/MediaPipe-FaceDetector-BlazeFace-Full-Range",
		// Converted from Google's TFLite by scripts/models/convert_blazeface.py,
		// which reproduces this hash.
		files: &[ModelFile {
			name: "blaze_face_full_range.onnx",
			url: "https://huggingface.co/spacedrive/spacedrive-models/resolve/da0ee1af007f12b788310d4ea8df2cf00f04fe44/blazeface-full-range/blaze_face_full_range.onnx",
			sha256: "494c597f51002a1ccbdc3571cc3d02a60fea914a7ad8abf6968e31768adf422a",
			size: 2_065_224,
		}],
	},
	ModelSpec {
		id: "auraface-v1",
		name: "AuraFace v1",
		category: Category::FaceEmbedding,
		engine: Engine::ArcFace,
		description: "Face recognition embedding trained on commercially licensed data. 261 MB.",
		license: "Apache-2.0",
		license_url: "https://huggingface.co/fal/AuraFace-v1",
		files: &[ModelFile {
			name: "glintr100.onnx",
			url: "https://huggingface.co/fal/AuraFace-v1/resolve/main/glintr100.onnx",
			sha256: "a7933ea5330113b01c9b60351d8f4c33003f145d8470ac5f0e52ee2effe25c60",
			size: 260_694_151,
		}],
	},
	ModelSpec {
		id: "siglip-base-patch16-224",
		name: "SigLIP base",
		category: Category::SceneClassification,
		engine: Engine::Siglip,
		description: "Zero-shot scene labels from Google's image-text model, 8-bit. 213 MB.",
		license: "Apache-2.0",
		license_url: "https://huggingface.co/google/siglip-base-patch16-224",
		files: &[
			ModelFile {
				name: "vision_model.onnx",
				url: "https://huggingface.co/Xenova/siglip-base-patch16-224/resolve/main/onnx/vision_model_quantized.onnx",
				sha256: "ef14a954f3d57e1806666432bd9785004c1dc27100aa260eee0cb0f10a5de058",
				size: 99_499_129,
			},
			ModelFile {
				name: "text_model.onnx",
				url: "https://huggingface.co/Xenova/siglip-base-patch16-224/resolve/main/onnx/text_model_quantized.onnx",
				sha256: "ad0329b1f35acc66d8953ff2559ce358da8eb0a7011794cf951523d63a4dbce2",
				size: 111_475_220,
			},
			ModelFile {
				name: "tokenizer.json",
				url: "https://huggingface.co/Xenova/siglip-base-patch16-224/resolve/main/tokenizer.json",
				sha256: "4a17c975210be5ab4c36b47d8dae4eefb866dbfb1e676e394aad85dc30a3ae08",
				size: 2_398_744,
			},
		],
	},
	ModelSpec {
		id: "multilingual-e5-small",
		name: "Multilingual E5 small",
		category: Category::TextEmbedding,
		engine: Engine::E5,
		description: "384-dimension sentence embeddings in about 100 languages, 8-bit. 135 MB.",
		license: "MIT",
		license_url: "https://huggingface.co/intfloat/multilingual-e5-small",
		files: &[
			ModelFile {
				name: "model.onnx",
				url: "https://huggingface.co/Xenova/multilingual-e5-small/resolve/main/onnx/model_quantized.onnx",
				sha256: "f80102d3f2a1229f387d3c81909990d8945513e347b0eab049f7de3c6f98c193",
				size: 118_308_185,
			},
			ModelFile {
				name: "tokenizer.json",
				url: "https://huggingface.co/Xenova/multilingual-e5-small/resolve/main/tokenizer.json",
				sha256: "0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39",
				size: 17_082_730,
			},
		],
	},
	ModelSpec {
		id: "florence-2-base-ft",
		name: "Florence-2 base",
		category: Category::ImageCaption,
		engine: Engine::Florence2,
		description: "Describes an image in a few sentences, on this device, 8-bit. 275 MB.",
		license: "MIT",
		license_url: "https://huggingface.co/microsoft/Florence-2-base-ft",
		// Pinned to a revision rather than main: the export is regenerated
		// upstream from time to time and every file has to come from one export.
		files: &[
			ModelFile {
				name: "vision_encoder.onnx",
				url: "https://huggingface.co/onnx-community/Florence-2-base-ft/resolve/e88a44eaf3791a35eae0c5a47b3dbcd36e67eb6f/onnx/vision_encoder_quantized.onnx",
				sha256: "3b79d54f23f666f731549db23cb070c35a979ce19cbd9720e90e67a78dc9768c",
				size: 93_746_540,
			},
			ModelFile {
				name: "embed_tokens.onnx",
				url: "https://huggingface.co/onnx-community/Florence-2-base-ft/resolve/e88a44eaf3791a35eae0c5a47b3dbcd36e67eb6f/onnx/embed_tokens_quantized.onnx",
				sha256: "6b2258db1c8ee9b160576ccde3cd3814d83a2edaed0dd1c6ca9ff3c38fa62214",
				size: 39_390_433,
			},
			ModelFile {
				name: "encoder_model.onnx",
				url: "https://huggingface.co/onnx-community/Florence-2-base-ft/resolve/e88a44eaf3791a35eae0c5a47b3dbcd36e67eb6f/onnx/encoder_model_quantized.onnx",
				sha256: "f4ad7a68f1fb875d3bcf735ea14a7021b7ba7e83baf7cf10289881b4ed6d9b85",
				size: 43_651_493,
			},
			ModelFile {
				name: "decoder_model_merged.onnx",
				url: "https://huggingface.co/onnx-community/Florence-2-base-ft/resolve/e88a44eaf3791a35eae0c5a47b3dbcd36e67eb6f/onnx/decoder_model_merged_quantized.onnx",
				sha256: "f22f52f980c33df0efa15932c2f3db6d9d3595ce6387eca938b8cfe23dc4c641",
				size: 98_177_697,
			},
			ModelFile {
				name: "tokenizer.json",
				url: "https://huggingface.co/onnx-community/Florence-2-base-ft/resolve/e88a44eaf3791a35eae0c5a47b3dbcd36e67eb6f/tokenizer.json",
				sha256: "d69dcdb2323e124ac4f800cb9863ddccea0d7bb11e16125e8df3bd60f2f8aeac",
				size: 2_297_961,
			},
		],
	},
];

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
