use nom::{
	branch::alt,
	bytes::complete::*,
	character::complete::*,
	combinator::*,
	error::{ErrorKind, ParseError},
	multi::*,
	sequence::*,
	AsChar, IResult, InputTakeAtPosition,
};

use super::{Attribute, AttributeFieldValue};

fn remove_ws<'a, O, E: ParseError<&'a str>, F>(
	wrapped: F,
) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
	F: FnMut(&'a str) -> IResult<&'a str, O, E>,
{
	delimited(multispace0, wrapped, multispace0)
}

fn parens(input: &str) -> IResult<&str, &str> {
	delimited(char('('), is_not(")"), char(')'))(input)
}

pub fn single_value<T, E: ParseError<T>>(i: T) -> IResult<T, T, E>
where
	T: InputTakeAtPosition,
	<T as InputTakeAtPosition>::Item: AsChar,
{
	i.split_at_position1_complete(
		|item| {
			let char_item = item.as_char();
			char_item != '_' && !char_item.is_alphanum()
		},
		ErrorKind::AlphaNumeric,
	)
}

fn list_value(input: &str) -> IResult<&str, Vec<&str>> {
	delimited(
		char('['),
		separated_list1(char(','), remove_ws(single_value)),
		char(']'),
	)(input)
}

fn attribute_field_value(input: &str) -> IResult<&str, AttributeFieldValue> {
	remove_ws(alt((
		map(list_value, AttributeFieldValue::List),
		map(single_value, AttributeFieldValue::Single),
	)))(input)
}

fn attribute_field(input: &str) -> IResult<&str, (&str, AttributeFieldValue)> {
	remove_ws(separated_pair(
		remove_ws(is_not(":")),
		char(':'),
		remove_ws(attribute_field_value),
	))(input)
}

fn attribute_fields(input: &str) -> IResult<&str, Vec<(&str, AttributeFieldValue)>> {
	separated_list1(char(','), attribute_field)(input)
}

pub fn parse(input: &str) -> IResult<&str, Attribute> {
	let (input, _) = remove_ws(tag("@"))(input)?;
	let (input, name) = alpha1(input)?;
	let (input, values_str) = opt(remove_ws(parens))(input)?;

	let fields = match values_str {
		Some(values_str) => attribute_fields(values_str)?.1,
		None => vec![],
	};

	Ok((input, Attribute { name, fields }))
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn marker() {
		let s = "@local";

		let (remaining, attribute) = super::parse(s).unwrap();

		assert_eq!(remaining, "");
		assert_eq!(attribute.name, "local");
		assert_eq!(attribute.fields.len(), 0);
	}

	#[test]
	fn single() {
		let s = "@local(foo: bar)";

		let (remaining, attribute) = super::parse(s).unwrap();

		assert_eq!(remaining, "");
		assert_eq!(attribute.name, "local");
		assert_eq!(attribute.fields.len(), 1);
		assert_eq!(attribute.fields[0].0, "foo");
		assert!(matches!(
			attribute.fields[0].1,
			AttributeFieldValue::Single("bar")
		));
	}

	#[test]
	fn list() {
		let s = "@local(foo: [bar, baz])";

		let (remaining, attribute) = match super::parse(s) {
			Ok(v) => v,
			Err(e) => panic!("{}", e),
		};

		assert_eq!(remaining, "");
		assert_eq!(attribute.name, "local");
		assert_eq!(attribute.fields.len(), 1);
		assert_eq!(attribute.fields[0].0, "foo");

		if let AttributeFieldValue::List(list) = &attribute.fields[0].1 {
			assert_eq!(list.len(), 2);
			assert_eq!(list[0], "bar");
			assert_eq!(list[1], "baz");
		} else {
			panic!("Expected list, got {:?}", attribute.fields[0].1);
		}
	}

	#[test]
	fn multiple() {
		let s = "@local(foo: bar, baz: qux)";

		let (remaining, attribute) = super::parse(s).unwrap();

		assert_eq!(remaining, "");
		assert_eq!(attribute.name, "local");
		assert_eq!(attribute.fields.len(), 2);
		assert_eq!(attribute.fields[0].0, "foo");
		assert!(matches!(
			attribute.fields[0].1,
			AttributeFieldValue::Single("bar")
		));
		assert_eq!(attribute.fields[1].0, "baz");
		assert!(matches!(
			attribute.fields[1].1,
			AttributeFieldValue::Single("qux")
		));
	}
}
