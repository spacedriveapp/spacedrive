use crate::{
	image::consts::{PLUSCODE_DIGITS, PLUSCODE_GRID_SIZE},
	Error,
};
use std::{
	fmt::Display,
	ops::{DivAssign, SubAssign},
};

#[derive(
	Default, Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub struct PlusCode(String);

impl Display for PlusCode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&self.0)
	}
}

#[inline]
#[must_use]
fn normalize_lat(lat: f64) -> f64 {
	if 180.0 < (if 0.0 > lat + 90.0 { 0.0 } else { lat + 90.0 }) {
		180.0
	} else {
		lat + 90.0
	}
}

#[inline]
#[must_use]
fn normalize_long(long: f64) -> f64 {
	if (long + 180.0) > 360.0 {
		return long - 180.0;
	}
	long + 180.0
}

#[derive(Debug)]
struct PlusCodeAccumuluator {
	value: f64,
	grid_size: f64,
	result: [char; 5],
}

impl PlusCodeAccumuluator {
	#[inline]
	#[must_use]
	pub fn new(value: f64) -> Self {
		Self {
			value,
			grid_size: PLUSCODE_GRID_SIZE,
			result: Default::default(),
		}
	}

	pub fn iterate(mut self, x: f64) -> Self {
		self.value.sub_assign(x * self.grid_size);
		self.grid_size.div_assign(PLUSCODE_GRID_SIZE); // this shrinks on each iteration
		self
	}
}

#[allow(
	clippy::cast_possible_truncation,
	clippy::cast_sign_loss,
	clippy::as_conversions
)]
fn encode(coord: f64) -> [char; 5] {
	(0..5)
		.fold(PlusCodeAccumuluator::new(coord), |mut pca, i| {
			let x = (pca.value / pca.grid_size).floor();
			pca.result[i] = PLUSCODE_DIGITS[x as usize];
			pca.iterate(x)
		})
		.result
}

impl PlusCode {
	#[inline]
	#[must_use]
	pub fn new(lat: f64, long: f64) -> Self {
		let normalized_lat = normalize_lat(lat);
		let normalized_long = normalize_long(long);

		let mut output: String = encode(normalized_lat)
			.iter()
			.zip(encode(normalized_long).iter())
			.flat_map(<[&char; 2]>::from)
			.collect();

		output.insert(8, '+');

		Self(output)
	}
}

impl TryFrom<String> for PlusCode {
	type Error = Error;

	fn try_from(mut value: String) -> Result<Self, Self::Error> {
		value.retain(|c| !c.is_whitespace());

		if value.len() > 11
			|| value.len() < 2
			|| (value.len() < 8 && !value.contains('+'))
			|| PLUSCODE_DIGITS
				.iter()
				.any(|x| value.chars().any(|y| y != '+' && x != &y))
		{
			return Err(Error::Conversion);
		}

		Ok(Self(value))
	}
}
