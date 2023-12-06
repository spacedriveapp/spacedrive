use std::sync::Arc;
use std::{collections::HashSet, path::Path};

use half::f16;
use image::{imageops::FilterType, GenericImageView};
use ndarray::{s, Array, Axis};
use ort::{inputs, SessionInputs};

use crate::skynet::utils::get_path_relative_to_exe;

use super::{ImageLabelerError, Model};

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

const MODEL_NAME: &str = "yolov8s.onnx";

impl YoloV8 {
	pub fn model(_: impl AsRef<Path>) -> Arc<dyn Model> {
		Arc::new(Self {
			model_path: get_path_relative_to_exe(Path::new(MODEL_LOCATION).join(MODEL_NAME))
				.into_boxed_path(),
		})
	}
}

impl Model for YoloV8 {
	fn path(&self) -> &Path {
		&self.model_path
	}

	fn prepare_input<'image>(
		&self,
		image: &'image [u8],
		format: image::ImageFormat,
	) -> Result<SessionInputs<'image>, ImageLabelerError> {
		let original_img = image::load_from_memory_with_format(image, format)?;
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
		output: ort::SessionOutputs<'_>,
	) -> Result<std::collections::HashSet<String>, crate::skynet::image_labeler::ImageLabelerError>
	{
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
			.collect::<HashSet<_>>()
			.into_iter()
			.map(ToString::to_string)
			.collect())
	}
}
