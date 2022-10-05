mod parser;

#[derive(Debug)]
pub enum AttributeFieldValue<'a> {
	Single(&'a str),
	List(Vec<&'a str>),
}

impl AttributeFieldValue<'_> {
	pub fn as_single(&self) -> Option<&str> {
		match self {
			AttributeFieldValue::Single(field) => Some(field),
			_ => None,
		}
	}

	// pub fn as_list(&self) -> Option<&Vec<&str>> {
	// 	match self {
	// 		AttributeFieldValue::List(fields) => Some(fields),
	// 		_ => None,
	// 	}
	// }
}

#[derive(Debug)]
pub struct Attribute<'a> {
	pub name: &'a str,
	pub fields: Vec<(&'a str, AttributeFieldValue<'a>)>,
}

impl<'a> Attribute<'a> {
	pub fn parse(input: &'a impl AsRef<str>) -> Result<Self, ()> {
		parser::parse(input.as_ref())
			.map(|(_, a)| a)
			.map_err(|_| ())
	}

	pub fn field(&self, name: &str) -> Option<&AttributeFieldValue> {
		self.fields.iter().find(|(n, _)| *n == name).map(|(_, v)| v)
	}
}
