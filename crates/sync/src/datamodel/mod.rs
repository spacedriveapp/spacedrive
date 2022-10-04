mod field;
mod model;

pub use field::*;
pub use model::*;

use crate::prelude::*;

pub const OPERATION_MODELS: &[&str] = &["OwnedOperation", "SharedOperation", "RelationOperation"];

pub struct Datamodel<'a> {
	pub prisma: &'a dml::Datamodel,
	pub models: Vec<Model<'a>>,
}

impl<'a> Datamodel<'a> {
	pub fn model(&self, name: &str) -> Option<&'a Model> {
		self.models.iter().find(|m| m.name == name)
	}
}

impl<'a> TryFrom<&'a dml::Datamodel> for Datamodel<'a> {
	type Error = String;
	fn try_from(datamodel: &'a dml::Datamodel) -> Result<Self, Self::Error> {
		let models = datamodel
			.models
			.iter()
			.filter(|m| !OPERATION_MODELS.contains(&m.name.as_str()))
			.map(|m| Model::new(m, datamodel))
			.collect::<Result<Vec<_>, _>>()?;

		let datamodel = Self {
			prisma: datamodel,
			models,
		};

		Ok(datamodel)
	}
}
