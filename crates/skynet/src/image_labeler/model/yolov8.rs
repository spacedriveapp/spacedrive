use crate::utils::get_path_relative_to_exe;

use std::{
	collections::{HashMap, HashSet},
	path::Path,
};

use half::f16;
use image::{imageops::FilterType, load_from_memory_with_format, GenericImageView, ImageFormat};
use ndarray::{s, Array, Axis};
use once_cell::sync::Lazy;
use ort::{inputs, SessionInputs, SessionOutputs};
use url::Url;

use super::{download_model, DownloadModelError, ImageLabelerError, Model, ModelOrigin};

pub struct YoloV8 {
	model_path: Box<Path>,
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

static MODEL_VERSIONS: Lazy<HashMap<&'static str, ModelOrigin>> = Lazy::new(|| {
	HashMap::from([
	("Yolo Nano", ModelOrigin::Url(Url::parse("https://github.com/spacedriveapp/native-deps/releases/download/yolo-2023-12-05/yolov8n.onnx").expect("Must be a valid URL"))),
	("Yolo Small", ModelOrigin::Path(get_path_relative_to_exe(Path::new(MODEL_LOCATION).join("yolov8s.onnx")))),
	("Yolo Medium", ModelOrigin::Url(Url::parse("https://github.com/spacedriveapp/native-deps/releases/download/yolo-2023-12-05/yolov8m.onnx").expect("Must be a valid URL"))),
	("Yolo Large", ModelOrigin::Url(Url::parse("https://github.com/spacedriveapp/native-deps/releases/download/yolo-2023-12-05/yolov8l.onnx").expect("Must be a valid URL"))),
	("Yolo Extra", ModelOrigin::Url(Url::parse("https://github.com/spacedriveapp/native-deps/releases/download/yolo-2023-12-05/yolov8x.onnx").expect("Must be a valid URL"))),
])
});

impl YoloV8 {
	pub async fn model(
		version: Option<&str>,
		data_dir: impl AsRef<Path>,
	) -> Result<Box<dyn Model>, DownloadModelError> {
		let model_path = if let Some(version) = version {
			download_model(
				MODEL_VERSIONS
					.get(version)
					.ok_or_else(|| DownloadModelError::UnknownModelVersion(version.to_string()))?,
				data_dir,
			)
			.await?
		} else {
			match MODEL_VERSIONS
				.get("Yolo Small")
				.expect("Default model version must be valid")
			{
				ModelOrigin::Path(path) => path.to_owned(),
				ModelOrigin::Url(_) => panic!("Defautl model must be an already existing path"),
			}
		};

		Ok(Box::new(Self {
			model_path: model_path.into_boxed_path(),
		}))
	}
}

impl Model for YoloV8 {
	fn path(&self) -> &Path {
		&self.model_path
	}

	fn versions(&self) -> Vec<&str> {
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
