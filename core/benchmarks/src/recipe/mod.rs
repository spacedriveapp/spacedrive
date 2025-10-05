mod schema;

pub use schema::*;

impl Recipe {
	pub fn name_str(&self) -> &str {
		&self.name
	}
}
