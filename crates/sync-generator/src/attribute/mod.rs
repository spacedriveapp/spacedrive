use prisma_client_rust_sdk::prisma::prisma_models::{ast::WithDocumentation, walkers::ModelWalker};

mod parser;

#[derive(Debug)]
pub enum AttributeFieldValue<'a> {
	Single(&'a str),
	List(Vec<&'a str>),
}

#[allow(unused)]
impl AttributeFieldValue<'_> {
	pub const fn as_single(&self) -> Option<&str> {
		if let AttributeFieldValue::Single(field) = self {
			Some(field)
		} else {
			None
		}
	}

	pub const fn as_list(&self) -> Option<&Vec<&str>> {
		if let AttributeFieldValue::List(fields) = self {
			Some(fields)
		} else {
			None
		}
	}
}

#[derive(Debug)]
pub struct Attribute<'a> {
	pub name: &'a str,
	pub fields: Vec<(&'a str, AttributeFieldValue<'a>)>,
}

impl<'a> Attribute<'a> {
	pub fn parse(input: &'a str) -> Result<Self, ()> {
		parser::parse(input).map(|(_, a)| a).map_err(|_| ())
	}

	pub fn field(&self, name: &str) -> Option<&AttributeFieldValue<'_>> {
		self.fields
			.iter()
			.find_map(|(n, v)| (*n == name).then_some(v))
	}
}

pub fn model_attributes(model: ModelWalker<'_>) -> Vec<Attribute<'_>> {
	model
		.ast_model()
		.documentation()
		.as_ref()
		.map(|docs| docs.lines().flat_map(Attribute::parse).collect())
		.unwrap_or_default()
}
