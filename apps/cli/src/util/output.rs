use serde::Serialize;

pub fn print_json<T: Serialize>(data: &T) {
	println!("{}", serde_json::to_string_pretty(data).unwrap());
}

/// Bytes in binary units, as `sd status` and `sd sources list` print them.
pub fn format_bytes(bytes: u64) -> String {
	const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
	let mut size = bytes as f64;
	let mut unit_index = 0;
	while size >= 1024.0 && unit_index < UNITS.len() - 1 {
		size /= 1024.0;
		unit_index += 1;
	}
	if unit_index == 0 {
		format!("{} {}", bytes, UNITS[unit_index])
	} else {
		format!("{:.1} {}", size, UNITS[unit_index])
	}
}

/// One replica fetch as a phrase: `553.0 MB of 2.9 GB, 212.0 KB/s`.
pub fn format_transfer(bytes: u64, total: u64, bytes_per_sec: u64) -> String {
	format!(
		"{} of {}, {}/s",
		format_bytes(bytes),
		format_bytes(total),
		format_bytes(bytes_per_sec)
	)
}
