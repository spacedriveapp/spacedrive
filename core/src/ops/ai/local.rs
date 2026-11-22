//! Models that run on this device through ONNX Runtime.
//!
//! Sessions use CoreML on macOS and fall back to the CPU for any operator
//! CoreML cannot place. Each session sits behind a mutex because a run needs
//! exclusive access; a batch is one run where the model has a batch axis.

use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use image::RgbImage;
use ort::environment::Environment;
use ort::session::Session;
use ort::value::Tensor;
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

use super::catalog::{Engine, ModelSpec, SCENE_LABELS};
use super::image as img;
use super::registry::{FaceDetector, FaceEmbedder, Provider, SceneClassifier, TextEmbedder};
use super::{AiError, BoundingBox, Face, Label};

pub fn load(spec: &ModelSpec, dir: &Path) -> Result<Provider, AiError> {
	let file = |name: &str| dir.join(name);
	let first = spec
		.files
		.first()
		.ok_or_else(|| AiError::Model(format!("{} lists no files", spec.id)))?;
	Ok(match spec.engine {
		Engine::BlazeFace => Provider::FaceDetection(Arc::new(BlazeFace::new(&file(first.name))?)),
		Engine::ArcFace => Provider::FaceEmbedding(Arc::new(ArcFace::new(&file(first.name))?)),
		Engine::Siglip => Provider::SceneClassification(Arc::new(Siglip::new(
			&file("vision_model.onnx"),
			&file("text_model.onnx"),
			&file("tokenizer.json"),
		)?)),
		Engine::E5 => Provider::TextEmbedding(Arc::new(E5::new(
			&file("model.onnx"),
			&file("tokenizer.json"),
		)?)),
	})
}

fn session(path: &Path) -> Result<Mutex<Session>, AiError> {
	let model = |e: ort::Error<_>| AiError::Model(format!("{}: {e}", path.display()));
	static ENVIRONMENT: OnceLock<Result<Environment, String>> = OnceLock::new();
	let environment = ENVIRONMENT
		.get_or_init(|| {
			ort::init()
				.with_name("spacedrive")
				.build()
				.map_err(|e| e.to_string())
		})
		.as_ref()
		.map_err(|e| AiError::Model(e.clone()))?;
	let builder = Session::builder(environment).map_err(|e| AiError::Model(e.to_string()))?;
	#[cfg(target_os = "macos")]
	let builder = builder
		.with_execution_providers([ort::ep::CoreML::default().build()])
		.map_err(model)?;
	let session = builder
		.with_intra_threads(num_threads())
		.map_err(model)?
		.commit_from_file(path)
		.map_err(|e| AiError::Model(format!("{}: {e}", path.display())))?;
	Ok(Mutex::new(session))
}

/// Inference shares the machine with indexing and the UI, so it takes half
/// the cores.
fn num_threads() -> usize {
	std::thread::available_parallelism()
		.map(|n| (n.get() / 2).max(1))
		.unwrap_or(1)
}

fn tokenizer(path: &Path, pad_to: Option<usize>, max_len: usize) -> Result<Tokenizer, AiError> {
	let mut tokenizer = Tokenizer::from_file(path)
		.map_err(|e| AiError::Model(format!("{}: {e}", path.display())))?;
	let pad_id = tokenizer
		.get_padding()
		.map(|p| (p.pad_id, p.pad_token.clone()))
		.or_else(|| {
			tokenizer
				.token_to_id("<pad>")
				.map(|id| (id, "<pad>".to_string()))
		})
		.unwrap_or((0, "<pad>".to_string()));
	tokenizer.with_padding(Some(PaddingParams {
		strategy: pad_to.map_or(PaddingStrategy::BatchLongest, PaddingStrategy::Fixed),
		pad_id: pad_id.0,
		pad_token: pad_id.1,
		..Default::default()
	}));
	tokenizer
		.with_truncation(Some(TruncationParams {
			max_length: max_len,
			..Default::default()
		}))
		.map_err(|e| AiError::Model(e.to_string()))?;
	Ok(tokenizer)
}

/// Runs a session and copies out the output named `output`, or the first
/// output when the model has no output by that name.
fn run(
	session: &Mutex<Session>,
	inputs: Vec<(&'static str, Tensor<i64>)>,
	floats: Option<(&'static str, Tensor<f32>)>,
	output: &str,
) -> Result<(Vec<i64>, Vec<f32>), AiError> {
	let mut session = session
		.lock()
		.map_err(|_| AiError::Model("session poisoned".into()))?;
	let names: Vec<String> = session
		.inputs()
		.iter()
		.map(|i| i.name().to_string())
		.collect();
	let mut values: Vec<(String, ort::value::DynValue)> = Vec::new();
	if let Some((name, tensor)) = floats {
		let name = names.first().cloned().unwrap_or_else(|| name.to_string());
		values.push((name, tensor.into_dyn()));
	}
	for (name, tensor) in inputs {
		if names.iter().any(|n| n == name) {
			values.push((name.to_string(), tensor.into_dyn()));
		}
	}
	let outputs = session.run(values)?;
	if outputs.len() == 0 {
		return Err(AiError::Model("model returned no output".into()));
	}
	let value = match (!output.is_empty()).then(|| outputs.get(output)).flatten() {
		Some(value) => value,
		None => &outputs[0],
	};
	let (shape, data) = value.try_extract_tensor::<f32>()?;
	Ok((shape.to_vec(), data.to_vec()))
}

fn normalize(v: &mut [f32]) {
	let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
	v.iter_mut().for_each(|x| *x /= norm);
}

/// MediaPipe BlazeFace full-range: one 192x192 frame, 2304 anchors on a
/// 48x48 grid, each regressing a box and six keypoints.
pub struct BlazeFace {
	session: Mutex<Session>,
}

const BLAZE_SIDE: u32 = 192;
const BLAZE_GRID: usize = 48;
const BLAZE_MIN_SCORE: f32 = 0.6;
const BLAZE_NMS_IOU: f32 = 0.3;

impl BlazeFace {
	pub fn new(path: &Path) -> Result<Self, AiError> {
		Ok(Self {
			session: session(path)?,
		})
	}
}

impl FaceDetector for BlazeFace {
	fn detect(&self, image: &RgbImage) -> Result<Vec<Face>, AiError> {
		let side = BLAZE_SIDE as usize;
		let (pixels, letterbox) = img::letterbox_nhwc(image, BLAZE_SIDE, -1.0, 1.0);
		let input = Tensor::from_array(([1usize, side, side, 3], pixels))?;

		let mut session = self
			.session
			.lock()
			.map_err(|_| AiError::Model("session poisoned".into()))?;
		let name = session.inputs()[0].name().to_string();
		let outputs = session.run(vec![(name, input.into_dyn())])?;
		let mut boxes = None;
		let mut scores = None;
		for (_, value) in outputs.iter() {
			let (shape, data) = value.try_extract_tensor::<f32>()?;
			match shape.last() {
				Some(16) => boxes = Some(data.to_vec()),
				Some(1) => scores = Some(data.to_vec()),
				_ => {}
			}
		}
		drop(outputs);
		drop(session);
		let (Some(boxes), Some(scores)) = (boxes, scores) else {
			return Err(AiError::Model("unexpected BlazeFace outputs".into()));
		};

		let scale = BLAZE_SIDE as f32;
		let mut candidates = Vec::new();
		for (i, raw) in scores.iter().enumerate() {
			let score = 1.0 / (1.0 + (-raw.clamp(-100.0, 100.0)).exp());
			if score < BLAZE_MIN_SCORE {
				continue;
			}
			let ax = ((i % BLAZE_GRID) as f32 + 0.5) / BLAZE_GRID as f32;
			let ay = ((i / BLAZE_GRID) as f32 + 0.5) / BLAZE_GRID as f32;
			let r = &boxes[i * 16..i * 16 + 16];
			let cx = r[0] / scale + ax;
			let cy = r[1] / scale + ay;
			let w = r[2] / scale;
			let h = r[3] / scale;
			let keypoints: Vec<(f32, f32)> = (0..6)
				.map(|k| (r[4 + k * 2] / scale + ax, r[5 + k * 2] / scale + ay))
				.collect();
			candidates.push((
				score,
				[cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0],
				keypoints,
			));
		}

		let (width, height) = image.dimensions();
		Ok(weighted_nms(candidates)
			.into_iter()
			.map(|(score, b, keypoints)| {
				let (x0, y0) = letterbox.unmap(b[0], b[1], width, height);
				let (x1, y1) = letterbox.unmap(b[2], b[3], width, height);
				let (x0, y0) = (x0.clamp(0.0, 1.0), y0.clamp(0.0, 1.0));
				let (x1, y1) = (x1.clamp(0.0, 1.0), y1.clamp(0.0, 1.0));
				Face {
					bbox: BoundingBox {
						x: x0,
						y: y0,
						width: x1 - x0,
						height: y1 - y0,
					},
					confidence: score,
					landmarks: keypoints
						.into_iter()
						.map(|(x, y)| {
							let (x, y) = letterbox.unmap(x, y, width, height);
							[x, y]
						})
						.collect(),
					embedding: Vec::new(),
					identified_as: None,
				}
			})
			.filter(|face| face.bbox.width > 0.0 && face.bbox.height > 0.0)
			.collect())
	}
}

type Candidate = (f32, [f32; 4], Vec<(f32, f32)>);

/// MediaPipe's weighted suppression: overlapping detections are averaged by
/// score instead of dropped, which steadies boxes from neighbouring anchors.
fn weighted_nms(mut candidates: Vec<Candidate>) -> Vec<Candidate> {
	candidates.sort_by(|a, b| b.0.total_cmp(&a.0));
	let mut out = Vec::new();
	while !candidates.is_empty() {
		let top = candidates[0].1;
		let (cluster, rest): (Vec<_>, Vec<_>) = candidates
			.into_iter()
			.partition(|c| iou(&top, &c.1) > BLAZE_NMS_IOU);
		candidates = rest;
		let total: f32 = cluster.iter().map(|c| c.0).sum();
		let mut b = [0.0; 4];
		let mut keypoints = vec![(0.0, 0.0); cluster[0].2.len()];
		for (score, cb, ck) in &cluster {
			let w = score / total;
			for i in 0..4 {
				b[i] += cb[i] * w;
			}
			for (k, p) in ck.iter().enumerate() {
				keypoints[k].0 += p.0 * w;
				keypoints[k].1 += p.1 * w;
			}
		}
		out.push((cluster[0].0, b, keypoints));
	}
	out
}

fn iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
	let w = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
	let h = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
	let inter = w * h;
	let union = (a[2] - a[0]) * (a[3] - a[1]) + (b[2] - b[0]) * (b[3] - b[1]) - inter;
	if union <= 0.0 {
		0.0
	} else {
		inter / union
	}
}

/// ArcFace-style embedding over a 112x112 crop aligned on the eyes, nose
/// and mouth.
pub struct ArcFace {
	session: Mutex<Session>,
}

const ARC_SIDE: u32 = 112;
/// The ArcFace reference points for the eyes, nose tip and mouth centre.
const ARC_TEMPLATE: [(f32, f32); 4] = [
	(38.2946, 51.6963),
	(73.5318, 51.5014),
	(56.0252, 71.7366),
	(56.1396, 92.2848),
];

impl ArcFace {
	pub fn new(path: &Path) -> Result<Self, AiError> {
		Ok(Self {
			session: session(path)?,
		})
	}

	fn embed_aligned(&self, crop: &RgbImage) -> Result<Vec<f32>, AiError> {
		let mut pixels = Vec::with_capacity((ARC_SIDE * ARC_SIDE * 3) as usize);
		img::push_nchw(&mut pixels, crop, ARC_SIDE, [0.5; 3], [0.5; 3]);
		let side = ARC_SIDE as usize;
		let input = Tensor::from_array(([1usize, 3, side, side], pixels))?;
		let (_, mut embedding) = run(&self.session, Vec::new(), Some(("data", input)), "")?;
		normalize(&mut embedding);
		Ok(embedding)
	}
}

impl FaceEmbedder for ArcFace {
	fn embed(&self, image: &RgbImage, face: &Face) -> Result<Vec<f32>, AiError> {
		if face.landmarks.len() < 4 {
			return Err(AiError::Input("face has no landmarks to align on".into()));
		}
		let (w, h) = (image.width() as f32, image.height() as f32);
		let src: Vec<(f32, f32)> = face.landmarks[..4]
			.iter()
			.map(|p| (p[0] * w, p[1] * h))
			.collect();
		self.embed_aligned(&align(image, &src))
	}

	fn embed_crop(&self, image: &RgbImage) -> Result<Vec<f32>, AiError> {
		self.embed_aligned(image)
	}
}

/// Warps `image` so the `src` points land on [`ARC_TEMPLATE`], with the
/// least-squares similarity transform (rotation, uniform scale, shift).
fn align(image: &RgbImage, src: &[(f32, f32)]) -> RgbImage {
	let n = src.len() as f32;
	let (smx, smy) = src
		.iter()
		.fold((0.0, 0.0), |a, p| (a.0 + p.0 / n, a.1 + p.1 / n));
	let (dmx, dmy) = ARC_TEMPLATE
		.iter()
		.fold((0.0, 0.0), |a, p| (a.0 + p.0 / n, a.1 + p.1 / n));
	let (mut num_a, mut num_b, mut den) = (0.0, 0.0, 0.0);
	for (s, d) in src.iter().zip(ARC_TEMPLATE.iter()) {
		let (sx, sy) = (s.0 - smx, s.1 - smy);
		let (dx, dy) = (d.0 - dmx, d.1 - dmy);
		num_a += sx * dx + sy * dy;
		num_b += sx * dy - sy * dx;
		den += sx * sx + sy * sy;
	}
	let (a, b) = if den > 0.0 {
		(num_a / den, num_b / den)
	} else {
		(1.0, 0.0)
	};
	let det = (a * a + b * b).max(1e-12);
	image::ImageBuffer::from_fn(ARC_SIDE, ARC_SIDE, |u, v| {
		let (dx, dy) = (u as f32 - dmx, v as f32 - dmy);
		let x = (a * dx + b * dy) / det + smx;
		let y = (-b * dx + a * dy) / det + smy;
		bilinear(image, x, y)
	})
}

fn bilinear(image: &RgbImage, x: f32, y: f32) -> image::Rgb<u8> {
	let (w, h) = (image.width() as i64, image.height() as i64);
	let (x0, y0) = (x.floor() as i64, y.floor() as i64);
	let (fx, fy) = (x - x0 as f32, y - y0 as f32);
	let px = |x: i64, y: i64| -> [f32; 3] {
		if x < 0 || y < 0 || x >= w || y >= h {
			return [0.0; 3];
		}
		let p = image.get_pixel(x as u32, y as u32);
		[p[0] as f32, p[1] as f32, p[2] as f32]
	};
	let (p00, p10, p01, p11) = (
		px(x0, y0),
		px(x0 + 1, y0),
		px(x0, y0 + 1),
		px(x0 + 1, y0 + 1),
	);
	let mut out = [0u8; 3];
	for c in 0..3 {
		let top = p00[c] * (1.0 - fx) + p10[c] * fx;
		let bottom = p01[c] * (1.0 - fx) + p11[c] * fx;
		out[c] = (top * (1.0 - fy) + bottom * fy).round().clamp(0.0, 255.0) as u8;
	}
	image::Rgb(out)
}

/// SigLIP zero-shot scene classification. The label prompts are embedded
/// once at load, so a batch costs one run of the vision tower.
pub struct Siglip {
	vision: Mutex<Session>,
	labels: Vec<Vec<f32>>,
}

const SIGLIP_SIDE: u32 = 224;
const SIGLIP_TEXT_LEN: usize = 64;
/// SigLIP base's learned logit scale, exp(4.765).
const SIGLIP_LOGIT_SCALE: f32 = 117.33;

impl Siglip {
	pub fn new(vision: &Path, text: &Path, tokenizer_path: &Path) -> Result<Self, AiError> {
		let tokenizer = tokenizer(tokenizer_path, Some(SIGLIP_TEXT_LEN), SIGLIP_TEXT_LEN)?;
		let text = session(text)?;
		let prompts: Vec<String> = SCENE_LABELS
			.iter()
			.map(|label| format!("a photo of a {label}."))
			.collect();
		let encodings = tokenizer
			.encode_batch(prompts, true)
			.map_err(|e| AiError::Model(e.to_string()))?;
		let ids: Vec<i64> = encodings
			.iter()
			.flat_map(|e| e.get_ids().iter().map(|&id| id as i64))
			.collect();
		let input = Tensor::from_array(([encodings.len(), SIGLIP_TEXT_LEN], ids))?;
		let (shape, data) = run(&text, vec![("input_ids", input)], None, "pooler_output")?;
		let dim = *shape.last().unwrap_or(&768) as usize;
		let labels = pooled(&shape, data, dim)
			.into_iter()
			.map(|mut v| {
				normalize(&mut v);
				v
			})
			.collect();
		Ok(Self {
			vision: session(vision)?,
			labels,
		})
	}
}

/// Rows of a pooled output; a [`N, L, D`] hidden state takes its first token.
fn pooled(shape: &[i64], data: Vec<f32>, dim: usize) -> Vec<Vec<f32>> {
	let stride = if shape.len() == 3 {
		shape[1] as usize * dim
	} else {
		dim
	};
	data.chunks(stride).map(|row| row[..dim].to_vec()).collect()
}

impl SceneClassifier for Siglip {
	fn classify(&self, images: &[RgbImage], top_k: usize) -> Result<Vec<Vec<Label>>, AiError> {
		if images.is_empty() {
			return Ok(Vec::new());
		}
		let side = SIGLIP_SIDE as usize;
		let mut pixels = Vec::with_capacity(images.len() * 3 * side * side);
		for image in images {
			img::push_nchw(&mut pixels, image, SIGLIP_SIDE, [0.5; 3], [0.5; 3]);
		}
		let input = Tensor::from_array(([images.len(), 3, side, side], pixels))?;
		let (shape, data) = run(
			&self.vision,
			Vec::new(),
			Some(("pixel_values", input)),
			"pooler_output",
		)?;
		let dim = *shape.last().unwrap_or(&768) as usize;
		Ok(pooled(&shape, data, dim)
			.into_iter()
			.map(|mut image| {
				normalize(&mut image);
				let logits: Vec<f32> = self
					.labels
					.iter()
					.map(|label| SIGLIP_LOGIT_SCALE * dot(&image, label))
					.collect();
				let max = logits.iter().copied().fold(f32::MIN, f32::max);
				let exp: Vec<f32> = logits.iter().map(|l| (l - max).exp()).collect();
				let total: f32 = exp.iter().sum();
				let mut ranked: Vec<Label> = exp
					.iter()
					.zip(SCENE_LABELS)
					.map(|(e, label)| Label {
						label: (*label).to_string(),
						confidence: e / total,
					})
					.collect();
				ranked.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
				ranked.truncate(top_k);
				ranked
			})
			.collect())
	}
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
	a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// E5 sentence embeddings, mean pooled over the attention mask.
pub struct E5 {
	session: Mutex<Session>,
	tokenizer: Tokenizer,
}

const E5_MAX_TOKENS: usize = 512;

impl E5 {
	pub fn new(model: &Path, tokenizer_path: &Path) -> Result<Self, AiError> {
		Ok(Self {
			session: session(model)?,
			tokenizer: tokenizer(tokenizer_path, None, E5_MAX_TOKENS)?,
		})
	}
}

impl TextEmbedder for E5 {
	fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, AiError> {
		if texts.is_empty() {
			return Ok(Vec::new());
		}
		// E5 is trained with a role prefix; "query: " suits symmetric search.
		let prefixed: Vec<String> = texts.iter().map(|t| format!("query: {t}")).collect();
		let encodings = self
			.tokenizer
			.encode_batch(prefixed, true)
			.map_err(|e| AiError::Model(e.to_string()))?;
		let len = encodings.first().map_or(0, |e| e.len());
		let flat = |f: &dyn Fn(&tokenizers::Encoding) -> Vec<i64>| -> Vec<i64> {
			encodings.iter().flat_map(f).collect()
		};
		let ids = flat(&|e| e.get_ids().iter().map(|&x| x as i64).collect());
		let mask = flat(&|e| e.get_attention_mask().iter().map(|&x| x as i64).collect());
		let types = flat(&|e| e.get_type_ids().iter().map(|&x| x as i64).collect());
		let shape = [encodings.len(), len];
		let (out_shape, hidden) = run(
			&self.session,
			vec![
				("input_ids", Tensor::from_array((shape, ids))?),
				("attention_mask", Tensor::from_array((shape, mask.clone()))?),
				("token_type_ids", Tensor::from_array((shape, types))?),
			],
			None,
			"last_hidden_state",
		)?;
		let dim = *out_shape.last().unwrap_or(&384) as usize;
		Ok((0..encodings.len())
			.map(|row| {
				let mut sum = vec![0.0f32; dim];
				let mut count = 0.0f32;
				for t in 0..len {
					if mask[row * len + t] == 0 {
						continue;
					}
					count += 1.0;
					let offset = (row * len + t) * dim;
					for (s, h) in sum.iter_mut().zip(&hidden[offset..offset + dim]) {
						*s += h;
					}
				}
				sum.iter_mut().for_each(|s| *s /= count.max(1.0));
				normalize(&mut sum);
				sum
			})
			.collect())
	}
}

/// Runs the real models when `SD_AI_MODELS_DIR` holds them (one directory
/// per catalog id) and `SD_AI_PHOTOS_DIR` holds the sample photos, so CI
/// without the downloads skips them.
#[cfg(test)]
mod tests {
	use super::*;
	use crate::ops::ai::catalog;

	fn dirs() -> Option<(std::path::PathBuf, std::path::PathBuf)> {
		Some((
			std::env::var_os("SD_AI_MODELS_DIR")?.into(),
			std::env::var_os("SD_AI_PHOTOS_DIR")?.into(),
		))
	}

	fn photo(dir: &Path, name: &str) -> RgbImage {
		img::decode(&std::fs::read(dir.join(name)).unwrap(), 2048).unwrap()
	}

	fn provider(models: &Path, id: &str) -> Provider {
		let spec = catalog::find(id).unwrap();
		load(spec, &models.join(id)).unwrap()
	}

	#[test]
	fn faces_are_found_and_embedded() {
		let Some((models, photos)) = dirs() else {
			return;
		};
		let Provider::FaceDetection(detector) = provider(&models, "blazeface-full-range") else {
			panic!("wrong provider");
		};
		let Provider::FaceEmbedding(embedder) = provider(&models, "auraface-v1") else {
			panic!("wrong provider");
		};
		let portrait = photo(&photos, "portrait.jpg");
		let family = photo(&photos, "family.jpg");
		let alone = detector.detect(&portrait).unwrap();
		let group = detector.detect(&family).unwrap();
		assert_eq!(alone.len(), 1, "{alone:?}");
		assert_eq!(group.len(), 4, "{group:?}");
		let reference = embedder.embed(&portrait, &alone[0]).unwrap();
		assert_eq!(reference.len(), 512);
		let mut similarities: Vec<f32> = group
			.iter()
			.map(|face| dot(&reference, &embedder.embed(&family, face).unwrap()))
			.collect();
		similarities.sort_by(|a, b| b.total_cmp(a));
		assert!(
			similarities[0] > 0.4 && similarities[1] < 0.3,
			"same person should stand out: {similarities:?}"
		);
	}

	#[test]
	fn scenes_are_labelled() {
		let Some((models, photos)) = dirs() else {
			return;
		};
		let Provider::SceneClassification(classifier) =
			provider(&models, "siglip-base-patch16-224")
		else {
			panic!("wrong provider");
		};
		let labels = classifier
			.classify(
				&[
					photo(&photos, "matterhorn.jpg"),
					photo(&photos, "beach.jpg"),
				],
				3,
			)
			.unwrap();
		let top: Vec<&str> = labels.iter().map(|l| l[0].label.as_str()).collect();
		assert!(
			["mountain", "glacier", "snow"].contains(&top[0]),
			"{labels:?}"
		);
		// Copacabana sits under mountains and towers, so beach need only rank.
		assert!(
			labels[1]
				.iter()
				.any(|l| ["beach", "coast"].contains(&l.label.as_str())),
			"{labels:?}"
		);
	}

	#[test]
	fn texts_embed_by_meaning() {
		let Some((models, _)) = dirs() else {
			return;
		};
		let Provider::TextEmbedding(embedder) = provider(&models, "multilingual-e5-small") else {
			panic!("wrong provider");
		};
		let v = embedder
			.embed(&[
				"a dog runs on the beach",
				"un perro corre en la playa",
				"quarterly tax report",
			])
			.unwrap();
		assert_eq!(v[0].len(), 384);
		assert!(
			dot(&v[0], &v[1]) > dot(&v[0], &v[2]),
			"{:?}",
			(dot(&v[0], &v[1]), dot(&v[0], &v[2]))
		);
	}
}
