use crate::utils::get_path_relative_to_exe;

use std::{
	collections::{HashMap, HashSet},
	fmt::Display,
	path::Path,
	sync::LazyLock,
};

use half::f16;
use image::{imageops::FilterType, load_from_memory_with_format, GenericImageView, ImageFormat};
use ndarray::{s, Array, Axis};
use ort::{inputs, SessionInputs, SessionOutputs};
use url::Url;

use super::{DownloadModelError, ImageLabelerError, Model, ModelSource};

pub struct YoloV8 {
	model_origin: &'static ModelSource,
	model_version: String,
}

// This path must be relative to the running binary
#[cfg(windows)]
const MODEL_LOCATION: &str = "./models";
#[cfg(unix)]
const MODEL_LOCATION: &str = if cfg!(target_os = "macos") {
	"../Frameworks/Spacedrive.framework/Resources/Models"
} else {
	"../share/spacedrive/models"
};

pub static DEFAULT_MODEL_VERSION: &str = "Yolo Small";

static MODEL_VERSIONS: LazyLock<HashMap<&'static str, ModelSource>> = LazyLock::new(|| {
	HashMap::from([
		("Yolo Nano", ModelSource::Url(Url::parse("https://github.com/spacedriveapp/native-deps/releases/download/yolo-2023-12-05/yolov8n.onnx").expect("Must be a valid URL"))),
		(DEFAULT_MODEL_VERSION, ModelSource::Path(get_path_relative_to_exe(Path::new(MODEL_LOCATION).join("yolov8s.onnx")))),
		("Yolo Medium", ModelSource::Url(Url::parse("https://github.com/spacedriveapp/native-deps/releases/download/yolo-2023-12-05/yolov8m.onnx").expect("Must be a valid URL"))),
		("Yolo Large", ModelSource::Url(Url::parse("https://github.com/spacedriveapp/native-deps/releases/download/yolo-2023-12-05/yolov8l.onnx").expect("Must be a valid URL"))),
		("Yolo Extra", ModelSource::Url(Url::parse("https://github.com/spacedriveapp/native-deps/releases/download/yolo-2023-12-05/yolov8x.onnx").expect("Must be a valid URL"))),
	])
});

impl YoloV8 {
	pub fn model<T>(version: Option<T>) -> Result<Box<dyn Model>, DownloadModelError>
	where
		T: AsRef<str> + Display,
	{
		let (model_version, model_origin) = match version {
			Some(version) => (
				version.to_string(),
				MODEL_VERSIONS
					.get(version.as_ref())
					.ok_or_else(|| DownloadModelError::UnknownModelVersion(version.to_string()))?,
			),
			None => {
				let version = DEFAULT_MODEL_VERSION;
				(
					version.to_string(),
					MODEL_VERSIONS
						.get(version)
						.expect("Default model version must be valid"),
				)
			}
		};

		Ok(Box::new(Self {
			model_origin,
			model_version,
		}))
	}
}

impl Model for YoloV8 {
	fn name(&self) -> &'static str {
		"YoloV8"
	}

	fn origin(&self) -> &'static ModelSource {
		self.model_origin
	}

	fn version(&self) -> &str {
		self.model_version.as_str()
	}

	fn versions() -> Vec<&'static str> {
		MODEL_VERSIONS.keys().copied().collect()
	}

	fn prepare_input<'image>(
		&self,
		path: &Path,
		image: &'image [u8],
		format: ImageFormat,
	) -> Result<SessionInputs<'image>, ImageLabelerError> {
		let original_img = load_from_memory_with_format(image, format)
			.map_err(|e| ImageLabelerError::ImageLoadFailed(e, path.into()))?;

		let img = original_img.resize_exact(640, 640, FilterType::CatmullRom);
		let mut input = Array::<f16, _>::zeros((1, 3, 640, 640));
		for pixel in img.pixels() {
			let x = pixel.0 as _;
			let y = pixel.1 as _;
			let [r, g, b, _] = pixel.2 .0;
			input[[0, 0, y, x]] = f16::from_f32((r as f32) / 255.);
			input[[0, 1, y, x]] = f16::from_f32((g as f32) / 255.);
			input[[0, 2, y, x]] = f16::from_f32((b as f32) / 255.);
		}

		inputs!["images" => input.view()]
			.map(Into::into)
			.map_err(Into::into)
	}

	fn process_output(
		&self,
		output: SessionOutputs<'_>,
	) -> Result<HashSet<String>, ImageLabelerError> {
		#[rustfmt::skip]
		const YOLOV8_CLASS_LABELS: [&str; 80] = [
			"person", "bicycle", "car", "motorcycle", "airplane", "bus", "train", "truck",
			"boat", "traffic light", "fire hydrant", "stop sign", "parking meter", "bench",
			"bird", "cat", "dog", "horse", "sheep", "cow", "elephant", "bear", "zebra",
			"giraffe", "backpack", "umbrella", "handbag", "tie", "suitcase", "frisbee",
			"skis", "snowboard", "sports ball", "kite", "baseball bat", "baseball glove",
			"skateboard", "surfboard", "tennis racket", "bottle", "wine glass", "cup",
			"fork", "knife", "spoon", "bowl", "banana", "apple", "sandwich", "orange",
			"broccoli", "carrot", "hot dog", "pizza", "donut", "cake", "chair", "couch",
			"potted plant", "bed", "dining table", "toilet", "tv", "laptop", "mouse",
			"remote", "keyboard", "cell phone", "microwave", "oven", "toaster", "sink",
			"refrigerator", "book", "clock", "vase", "scissors", "teddy bear",
			"hair drier", "toothbrush"
		];

		let output0 = &output["output0"];

		let output_tensor = output0.extract_tensor::<f16>()?;

		let output_view = output_tensor.view();

		let output_tensor_transposed = output_view.t();

		let output = output_tensor_transposed.slice(s![.., .., 0]);

		Ok(output
			.axis_iter(Axis(0))
			.map(|row| {
				row.iter()
					// skip bounding box coordinates
					.skip(4)
					.enumerate()
					.map(|(class_id, probability)| (class_id, *probability))
					.reduce(|accum, row| if row.1 > accum.1 { row } else { accum })
					.expect("not empty output")
			})
			.filter(|(_, probability)| probability.to_f32() > 0.6)
			.map(|(class_id, _)| YOLOV8_CLASS_LABELS[class_id])
			.fold(HashSet::default(), |mut set, label| {
				if !set.contains(label) {
					set.insert(label.to_string());
				}

				set
			}))
	}
}
