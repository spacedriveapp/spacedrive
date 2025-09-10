use crate::context::OutputFormat;

pub fn print_human_line(line: &str) { println!("{}", line) }

pub fn print_json<T: serde::Serialize>(value: &T) {
	println!("{}", serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string()))
}


