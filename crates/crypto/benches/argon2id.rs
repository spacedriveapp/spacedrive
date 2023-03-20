use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use sd_crypto::{
	keys::Hasher,
	primitives::generate_bytes,
	types::{HashingAlgorithm, Params, Salt},
	Protected,
};

const PARAMS: [Params; 3] = [Params::Standard, Params::Hardened, Params::Paranoid];

fn bench(c: &mut Criterion) {
	let mut group = c.benchmark_group("argon2id");
	group.sample_size(10);

	for param in PARAMS {
		let password = Protected::new(generate_bytes(32));
		let salt = Salt::generate();
		let hashing_algorithm = HashingAlgorithm::Argon2id(param);

		group.bench_function(
			BenchmarkId::new("hash", hashing_algorithm.get_parameters().0),
			|b| {
				b.iter_batched(
					|| (password.clone(), salt),
					|(password, salt)| Hasher::hash(hashing_algorithm, password, salt, None),
					BatchSize::SmallInput,
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
