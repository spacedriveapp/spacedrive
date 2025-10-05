use serde::Serialize;

pub fn print_json<T: Serialize>(data: &T) {
	println!("{}", serde_json::to_string_pretty(data).unwrap());
}
