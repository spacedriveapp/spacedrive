//! # Hardware AI Capabilities
//!
//! Maps known hardware (CPUs and GPUs) to their AI compute capabilities measured in TOPS
//! (Trillion Operations Per Second). The values are typically FP16 precision for GPUs
//! (Tensor Core operations) and INT8 for NPUs (Neural Engine).

use serde::{Deserialize, Serialize};
use specta::Type;

/// AI compute capabilities for a hardware component
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct HardwareAICapabilities {
	/// AI compute power in TOPS (INT8 precision, typical for NPUs)
	pub tops_int8: Option<f64>,
	/// AI compute power in TOPS (FP16 precision, typical for GPU Tensor Cores)
	pub tops_fp16: Option<f64>,
}

/// Look up AI compute capabilities for known hardware and return total TOPS.
///
/// Combines NPU (from CPU model) and GPU AI acceleration capabilities. For Apple Silicon,
/// the Neural Engine is identified via CPU model. For discrete GPUs, Tensor Core (NVIDIA)
/// or Matrix Engine (AMD) TOPS are used.
pub fn lookup_ai_capabilities(
	cpu_model: Option<&str>,
	gpu_models: Option<&Vec<String>>,
) -> Option<f64> {
	let mut total_tops = 0.0;

	// NPU/Neural Engine capabilities from CPU model
	if let Some(cpu) = cpu_model {
		if let Some(tops) = get_cpu_ai_tops(cpu) {
			total_tops += tops;
		}
	}

	// GPU AI accelerator capabilities
	if let Some(gpus) = gpu_models {
		for gpu in gpus {
			if let Some(tops) = get_gpu_ai_tops(gpu) {
				total_tops += tops;
			}
		}
	}

	if total_tops > 0.0 {
		Some(total_tops)
	} else {
		None
	}
}

/// Get Neural Engine / NPU TOPS from CPU model string.
///
/// Apple Silicon Neural Engine values are official specs. Intel NPU and AMD Ryzen AI
/// values are from manufacturer specifications for their respective AI accelerators.
fn get_cpu_ai_tops(model: &str) -> Option<f64> {
	let model_lower = model.to_lowercase();

	// Apple Silicon Neural Engine TOPS (INT8)
	// M4 series uses 16-core Neural Engine
	if model_lower.contains("m4 ultra") {
		Some(76.0)
	} else if model_lower.contains("m4 max") {
		Some(38.0)
	} else if model_lower.contains("m4 pro") {
		Some(38.0)
	} else if model_lower.contains("m4") {
		Some(38.0)
	}
	// M3 series
	else if model_lower.contains("m3 ultra") {
		Some(70.0)
	} else if model_lower.contains("m3 max") {
		Some(35.0)
	} else if model_lower.contains("m3 pro") {
		Some(18.0)
	} else if model_lower.contains("m3") {
		Some(18.0)
	}
	// M2 series
	else if model_lower.contains("m2 ultra") {
		Some(31.6)
	} else if model_lower.contains("m2 max") {
		Some(15.8)
	} else if model_lower.contains("m2 pro") {
		Some(15.8)
	} else if model_lower.contains("m2") {
		Some(15.8)
	}
	// M1 series
	else if model_lower.contains("m1 ultra") {
		Some(22.0)
	} else if model_lower.contains("m1 max") {
		Some(11.0)
	} else if model_lower.contains("m1 pro") {
		Some(11.0)
	} else if model_lower.contains("m1") {
		Some(11.0)
	}
	// A-series chips (iOS)
	else if model_lower.contains("a17 pro") {
		Some(35.0)
	} else if model_lower.contains("a16") {
		Some(17.0)
	} else if model_lower.contains("a15") {
		Some(15.8)
	} else if model_lower.contains("a14") {
		Some(11.0)
	}
	// Intel Core Ultra (Meteor Lake and newer with NPU)
	else if model_lower.contains("core ultra") {
		// Lunar Lake (Series 2) has 48 TOPS NPU
		if model_lower.contains("series 2") || model_lower.contains("200") {
			Some(48.0)
		}
		// Arrow Lake and Meteor Lake (Series 1) have ~10-13 TOPS NPU
		else if model_lower.contains("series 1") || model_lower.contains("100") {
			Some(10.0)
		}
		// Default for unspecified Core Ultra
		else {
			Some(10.0)
		}
	}
	// AMD Ryzen AI (XDNA NPU)
	else if model_lower.contains("ryzen ai") {
		// Strix Point (Ryzen AI 300 series) has 50 TOPS NPU
		if model_lower.contains("300") || model_lower.contains("9 hx 370") {
			Some(50.0)
		}
		// Hawk Point (Ryzen 8000 series) has 16 TOPS NPU
		else {
			Some(16.0)
		}
	}
	// AMD Ryzen 8000/9000 series with XDNA
	else if model_lower.contains("ryzen 9 8") || model_lower.contains("ryzen 7 8") {
		Some(16.0)
	}
	// Qualcomm Snapdragon X series (for Windows on ARM)
	else if model_lower.contains("snapdragon x elite") {
		Some(45.0)
	} else if model_lower.contains("snapdragon x plus") {
		Some(45.0)
	} else {
		None
	}
}

/// Get GPU AI accelerator TOPS from GPU model string.
///
/// NVIDIA values are FP16 Tensor Core TOPS. AMD values are FP16 Matrix Engine TOPS.
/// Intel Arc values are XMX engine TOPS.
fn get_gpu_ai_tops(model: &str) -> Option<f64> {
	let model_lower = model.to_lowercase();

	// NVIDIA RTX 50 series (Blackwell, FP16 Tensor TOPS)
	if model_lower.contains("rtx 5090") {
		Some(3352.0)
	} else if model_lower.contains("rtx 5080") {
		Some(1801.0)
	} else if model_lower.contains("rtx 5070 ti") {
		Some(1406.0)
	} else if model_lower.contains("rtx 5070") {
		Some(988.0)
	}
	// NVIDIA RTX 40 series (Ada Lovelace, FP16 Tensor TOPS)
	else if model_lower.contains("rtx 4090") {
		Some(1321.0)
	} else if model_lower.contains("rtx 4080 super") {
		Some(836.0)
	} else if model_lower.contains("rtx 4080") {
		Some(780.0)
	} else if model_lower.contains("rtx 4070 ti super") {
		Some(568.0)
	} else if model_lower.contains("rtx 4070 ti") {
		Some(480.0)
	} else if model_lower.contains("rtx 4070 super") {
		Some(484.0)
	} else if model_lower.contains("rtx 4070") {
		Some(392.0)
	} else if model_lower.contains("rtx 4060 ti") {
		Some(242.0)
	} else if model_lower.contains("rtx 4060") {
		Some(242.0)
	}
	// NVIDIA RTX 30 series (Ampere, FP16 Tensor TOPS)
	else if model_lower.contains("rtx 3090 ti") {
		Some(320.0)
	} else if model_lower.contains("rtx 3090") {
		Some(285.0)
	} else if model_lower.contains("rtx 3080 ti") {
		Some(272.0)
	} else if model_lower.contains("rtx 3080") {
		Some(238.0)
	} else if model_lower.contains("rtx 3070 ti") {
		Some(174.0)
	} else if model_lower.contains("rtx 3070") {
		Some(163.0)
	} else if model_lower.contains("rtx 3060 ti") {
		Some(130.0)
	} else if model_lower.contains("rtx 3060") {
		Some(101.0)
	}
	// NVIDIA RTX 20 series (Turing, FP16 Tensor TOPS)
	else if model_lower.contains("rtx 2080 ti") {
		Some(107.6)
	} else if model_lower.contains("rtx 2080 super") {
		Some(89.2)
	} else if model_lower.contains("rtx 2080") {
		Some(80.5)
	} else if model_lower.contains("rtx 2070 super") {
		Some(72.5)
	} else if model_lower.contains("rtx 2070") {
		Some(59.7)
	} else if model_lower.contains("rtx 2060 super") {
		Some(57.4)
	} else if model_lower.contains("rtx 2060") {
		Some(51.6)
	}
	// AMD Radeon RX 7000 series (RDNA 3, FP16 Matrix TOPS)
	else if model_lower.contains("rx 7900 xtx") {
		Some(123.0)
	} else if model_lower.contains("rx 7900 xt") {
		Some(103.0)
	} else if model_lower.contains("rx 7900 gre") {
		Some(92.0)
	} else if model_lower.contains("rx 7800 xt") {
		Some(74.0)
	} else if model_lower.contains("rx 7700 xt") {
		Some(70.0)
	} else if model_lower.contains("rx 7600 xt") {
		Some(45.0)
	} else if model_lower.contains("rx 7600") {
		Some(43.0)
	}
	// AMD Radeon RX 6000 series (RDNA 2)
	else if model_lower.contains("rx 6950 xt") {
		Some(47.3)
	} else if model_lower.contains("rx 6900 xt") {
		Some(46.1)
	} else if model_lower.contains("rx 6800 xt") {
		Some(41.5)
	} else if model_lower.contains("rx 6800") {
		Some(32.3)
	} else if model_lower.contains("rx 6700 xt") {
		Some(26.4)
	}
	// Intel Arc series (XMX AI TOPS)
	else if model_lower.contains("arc a770") {
		Some(138.0)
	} else if model_lower.contains("arc a750") {
		Some(110.0)
	} else if model_lower.contains("arc a580") {
		Some(89.0)
	} else if model_lower.contains("arc a380") {
		Some(49.0)
	} else if model_lower.contains("arc a310") {
		Some(37.0)
	}
	// Apple Silicon integrated GPU (combined with Neural Engine, already counted in CPU)
	// Return None to avoid double-counting
	else if model_lower.contains("apple m") {
		None
	} else {
		None
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_apple_silicon_m4_pro() {
		let cpu = Some("Apple M4 Pro");
		let result = lookup_ai_capabilities(cpu, None);
		assert_eq!(result, Some(38.0));
	}

	#[test]
	fn test_apple_silicon_m3_max() {
		let cpu = Some("Apple M3 Max");
		let result = lookup_ai_capabilities(cpu, None);
		assert_eq!(result, Some(35.0));
	}

	#[test]
	fn test_nvidia_rtx_4090() {
		let gpus = Some(vec!["NVIDIA GeForce RTX 4090".to_string()]);
		let result = lookup_ai_capabilities(None, gpus.as_ref());
		assert_eq!(result, Some(1321.0));
	}

	#[test]
	fn test_combined_intel_with_nvidia() {
		let cpu = Some("Intel Core Ultra 7 155H");
		let gpus = Some(vec!["NVIDIA GeForce RTX 4070".to_string()]);
		let result = lookup_ai_capabilities(cpu, gpus.as_ref());
		// 10 TOPS (Intel NPU) + 392 TOPS (RTX 4070)
		assert_eq!(result, Some(402.0));
	}

	#[test]
	fn test_amd_ryzen_ai() {
		let cpu = Some("AMD Ryzen AI 9 HX 370");
		let result = lookup_ai_capabilities(cpu, None);
		assert_eq!(result, Some(50.0));
	}

	#[test]
	fn test_unknown_hardware() {
		let cpu = Some("Intel Core i5-10400");
		let gpus = Some(vec!["Intel UHD Graphics 630".to_string()]);
		let result = lookup_ai_capabilities(cpu, gpus.as_ref());
		assert_eq!(result, None);
	}

	#[test]
	fn test_case_insensitive() {
		let cpu = Some("APPLE M4 MAX");
		let result = lookup_ai_capabilities(cpu, None);
		assert_eq!(result, Some(38.0));
	}

	#[test]
	fn test_multi_gpu() {
		let gpus = Some(vec![
			"NVIDIA GeForce RTX 3080".to_string(),
			"NVIDIA GeForce RTX 3080".to_string(),
		]);
		let result = lookup_ai_capabilities(None, gpus.as_ref());
		// 238 + 238 = 476 TOPS
		assert_eq!(result, Some(476.0));
	}
}
