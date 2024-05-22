use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use sd_crypto::{
	hashing::Hasher,
	rng::CryptoRng,
	types::{HashingAlgorithm, Params, Salt, SecretKey},
	Protected,
};

const PARAMS: [Params; 3] = [Params::Standard, Params::Hardened, Params::Paranoid];

fn bench(c: &mut Criterion) {
	let mut group = c.benchmark_group("argon2id");
	group.sample_size(10); // TODO(brxken128): probably remove this

	for param in PARAMS {
		let password: Protected<Vec<u8>> = CryptoRng::generate_vec(16).into();
		let salt = Salt::generate();
		let hashing_algorithm = HashingAlgorithm::Argon2id(param);

		group.bench_function(
			BenchmarkId::new("hash", hashing_algorithm.get_parameters().0),
			|b| {
				b.iter_batched(
					|| (password.clone(), salt),
					|(password, salt)| {
						Hasher::hash_password(hashing_algorithm, &password, salt, &SecretKey::Null)
					},
					BatchSize::LargeInput,
				)
			},
		);
	}

	group.finish();
}

criterion_group!(
	name = benches;
	config = Criterion::default();
	targets = bench
);

criterion_main!(benches);
