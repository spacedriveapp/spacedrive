// use super::CryptoRng;
// use rand::Rng;
// use zeroize::{Zeroize, ZeroizeOnDrop};

// pub const WORDS: &str = include_str!("../.././assets/eff_large_wordlist.txt");

// #[derive(Default, Clone, Copy)]
// pub enum MnemonicDelimiter {
// 	#[default]
// 	None,
// 	Dash,
// 	Period,
// 	Comma,
// 	CommaSpace,
// }

// #[derive(Zeroize, ZeroizeOnDrop, Clone)]
// pub struct Mnemonic(Vec<String>);

// impl Mnemonic {
// 	#[must_use]
// 	fn get_all_words<'a>() -> Vec<&'a str> {
// 		WORDS.lines().collect()
// 	}

// 	#[must_use]
// 	pub fn generate_word() -> Self {
// 		let index = CryptoRng::from_entropy().gen_range(0..=WORDS.len());
// 		Self(vec![Self::get_all_words()[index].to_string()])
// 	}

// 	#[must_use]
// 	pub fn generate_mnemonic(delimiter: MnemonicDelimiter) -> Self {
// 		todo!()
// 	}
// }

// #[cfg(test)]
// mod tests {
// 	use crate::{ct::ConstantTimeEqNull, primitives::SALT_LEN, rng::CryptoRng};

// 	#[test]
// 	fn generate_bytes() {
// 		let bytes = CryptoRng::generate_vec(SALT_LEN);
// 		let bytes2 = CryptoRng::generate_vec(SALT_LEN);

// 		assert!(!bool::from(bytes.ct_eq_null()));
// 		assert_ne!(bytes, bytes2);
// 		assert_eq!(bytes.len(), SALT_LEN);
// 		assert_eq!(bytes2.len(), SALT_LEN);
// 	}

// 	#[test]
// 	fn generate_fixed() {
// 		let bytes: [u8; SALT_LEN] = CryptoRng::generate_fixed();
// 		let bytes2: [u8; SALT_LEN] = CryptoRng::generate_fixed();

// 		assert!(!bool::from(bytes.ct_eq_null()));
// 		assert_ne!(bytes, bytes2);
// 		assert_eq!(bytes.len(), SALT_LEN);
// 		assert_eq!(bytes2.len(), SALT_LEN);
// 	}
// }
// =================================================================
// // #[derive(Zeroize, ZeroizeOnDrop, Clone)]
// // pub struct Mnemonic<const I: usize>([String; I]);

// // impl<const I: usize> Mnemonic<I> {
// // 	#[must_use]
// // 	fn get_all_words<'a>() -> Vec<&'a str> {
// // 		WORDS.lines().collect()
// // 	}

// // 	pub fn generate_word() -> Result<Mnemonic<1>> {
// // 		// let i = index::sample(&mut CryptoRng::from_entropy(), WORDS.len(), 1);
// // 		// Ok(Mnemonic::get_all_words()[i.index(0)]))
// // 		let index = CryptoRng::from_entropy().gen_range(0..=WORDS.len());
// // 		Self([Self::get_all_words()[index].to_string()])
// // 	}
// // 	pub fn generate_mnemonic(delimiter: MnemonicDelimiter) -> Result<Mnemonic<I>> {
// // 		todo!()
// // 	}
// // }

// // #[must_use]
// // pub fn generate_passphrase(len: usize, delimiter: Option<char>) -> Protected<String> {
// // 	let words: Vec<&str> = WORDS.lines().collect();
// // 	let mut output = String::new();

// // 	let mut rng = CryptoRng::from_entropy();
// // 	let indexes = index::sample(&mut rng, words.len(), len);

// // 	indexes.iter().for_each(|i| {
// // 		output.push_str(words[i]);
// // 		if i < len - 1 && len != 1 {
// // 			if let Some(delimiter) = delimiter {
// // 				output.push(delimiter);
// // 			}
// // 		}
// // 	});

// // 	Protected::new(output)
// // }
