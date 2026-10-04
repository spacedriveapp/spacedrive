use crate::{
	exif::consts::{PLUSCODE_DIGITS, PLUSCODE_GRID_SIZE},
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
	#[allow(clippy::tuple_array_conversions)]
	pub fn new(lat: f64, long: f64) -> Self {
		let mut output = Self::encode_coordinates(Self::normalize_lat(lat))
			.into_iter()
			.zip(Self::encode_coordinates(Self::normalize_long(long)))
			.flat_map(|(x, y)| [x, y])
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

	fn try_from(value: String) -> Result<Self, Self::Error> {
		let value = value.trim();
		// Google shows short codes with a locality after them ("WR2C+2C Bibra Lake");
		// only the first token is the code, the rest is free text.
		let code = value.split_whitespace().next().ok_or(Error::Conversion)?;

		if !Self::is_valid(code) {
			return Err(Error::Conversion);
		}

		Ok(Self(value.to_string()))
	}
}

impl PlusCode {
	/// Code validation per the Open Location Code specification.
	///
	/// A code has exactly one `+` separator at an even index no later than 8.
	/// Short codes drop 2 to 8 leading characters, so the separator can sit at
	/// 0, 2, 4 or 6. Zero padding may only appear before the separator in a
	/// full code: it starts at an even index of at least 2, runs up to the
	/// separator and nothing follows the separator. The part after the
	/// separator is empty or at least two characters, and every other character
	/// comes from the 20 character alphabet, in either case.
	fn is_valid(code: &str) -> bool {
		if !code.is_ascii() {
			return false;
		}

		let mut separators = code.match_indices('+').map(|(i, _)| i);
		let Some(separator) = separators.next() else {
			return false;
		};
		if separators.next().is_some() || separator > 8 || separator % 2 != 0 {
			return false;
		}

		let prefix = &code[..separator];
		let suffix = &code[separator + 1..];
		if suffix.len() == 1 || (prefix.is_empty() && suffix.is_empty()) {
			return false;
		}

		let digits = match prefix.find('0') {
			Some(pad) => {
				if pad < 2
					|| pad % 2 != 0 || separator != 8
					|| !suffix.is_empty()
					|| !prefix[pad..].bytes().all(|b| b == b'0')
				{
					return false;
				}
				&prefix[..pad]
			}
			None => prefix,
		};

		digits
			.chars()
			.chain(suffix.chars())
			.all(|c| PLUSCODE_DIGITS.contains(&c.to_ascii_uppercase()))
	}
}

#[cfg(test)]
mod tests {
	use super::PlusCode;

	#[test]
	fn pluscode_maximum_precision() {
		let x = String::from("8FW4V74V+X8");
		PlusCode::try_from(x).unwrap();
	}

	#[test]
	fn pluscode_google() {
		let x = String::from("WR2C+2C Bibra Lake");
		PlusCode::try_from(x).unwrap();
	}

	#[test]
	fn pluscode_accepts_spec_forms() {
		for code in [
			"8FW4V74V+",
			"8FW4V74V+X8",
			"8FW4V74V+X8Q",
			"8FW4V74V+X8QRGHJ",
			"8FVC0000+",
			"8F000000+",
			"8fw4v74v+x8",
			"+X8",
			"4V+X8",
		] {
			PlusCode::try_from(code.to_string()).unwrap_or_else(|_| panic!("{code} rejected"));
		}
	}

	#[test]
	fn pluscode_rejects_invalid_forms() {
		for code in [
			"",
			"+",
			"8FW4V74VX8",
			"8FW4V74V+X8+",
			"8FW4V74V+X",
			"8FW4V74+VX8",
			"8FW4V74VX8+Q",
			"8FW4V74V+X0",
			"8FVC000+",
			"8F0C0000+",
			"8FVC0000+X8",
			"WR00+",
			"8FW4V74V+XA",
		] {
			assert!(
				PlusCode::try_from(code.to_string()).is_err(),
				"{code} accepted"
			);
		}
	}

	#[test]
	fn pluscode_roundtrips_the_encoder() {
		let encoded = PlusCode::new(-32.1, 115.8).to_string();
		assert_eq!(
			PlusCode::try_from(encoded.clone()).unwrap().to_string(),
			encoded
		);
	}
}
