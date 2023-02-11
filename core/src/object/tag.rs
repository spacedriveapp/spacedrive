// WIP

#[derive(Type, Deserialize)]
pub struct Tag {
	pub name: String,
	pub color: String,
}

impl Tag {
	pub fn create(name: String, color: String) -> Self {
		let Self { name, color };

		Self
	}
}
