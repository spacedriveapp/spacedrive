use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct SubtitleProps {
	pub width: i32,
	pub height: i32,
}
