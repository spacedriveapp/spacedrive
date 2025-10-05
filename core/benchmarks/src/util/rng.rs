use rand::{rngs::StdRng, SeedableRng};

pub fn rng_from_optional_seed(seed: Option<u64>) -> StdRng {
	match seed {
		Some(s) => StdRng::seed_from_u64(s),
		None => StdRng::from_entropy(),
	}
}
