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

struct PlusCodeState {
	coord_state: f64,
	grid_size: f64,
	result: [char; 5],
}

impl PlusCodeState {
	#[inline]
	#[must_use]
	fn new(coord_state: f64) -> Self {
		Self {
			coord_state,
			grid_size: PLUSCODE_GRID_SIZE,
			result: Default::default(),
		}
	}

	#[inline]
	#[must_use]
	fn iterate(mut self, x: f64) -> Self {
		self.coord_state.sub_assign(x * self.grid_size);
		self.grid_size.div_assign(PLUSCODE_GRID_SIZE); // this shrinks on each iteration
		self
	}
}

impl PlusCode {
	#[inline]
	#[must_use]
	pub fn new(lat: f64, long: f64) -> Self {
		let mut output = Self::encode_coordinates(Self::normalize_lat(lat))
			.into_iter()
			.zip(Self::encode_coordinates(Self::normalize_long(long)))
			.flat_map(<[char; 2]>::from)
			.collect::<String>();
		output.insert(8, '+');

		Self(output)
	}

	#[allow(
		clippy::cast_possible_truncation,
		clippy::cast_sign_loss,
		clippy::as_conversions
	)]
	#[inline]
	#[must_use]
	fn encode_coordinates(coordinates: f64) -> [char; 5] {
		(0..5)
			.fold(PlusCodeState::new(coordinates), |mut pcs, i| {
				let x = (pcs.coord_state / pcs.grid_size).floor();
				pcs.result[i] = PLUSCODE_DIGITS[x as usize];
				pcs.iterate(x)
			})
			.result
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
