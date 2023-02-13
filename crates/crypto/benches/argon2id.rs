// use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
// use sd_crypto::{
// 	keys::hashing::{HashingAlgorithm, Params},
// 	primitives::{generate_master_key, generate_salt},
// 	Protected,
// };

// const PARAMS: [Params; 3] = [Params::Standard, Params::Hardened, Params::Paranoid];

// fn bench(c: &mut Criterion) {
// 	let mut group = c.benchmark_group("argon2id");

// 	for param in PARAMS {
// 		let key = Protected::new(generate_master_key().expose().to_vec());
// 		let salt = generate_salt();
// 		let hashing_algorithm = HashingAlgorithm::Argon2id(param);

// 		group.bench_function(BenchmarkId::new("hash", param.argon2id().m_cost()), |b| {
// 			b.iter_batched(
// 				|| (key.clone(), salt),
// 				|(key, salt)| hashing_algorithm.hash(key, salt, None),
// 				BatchSize::SmallInput,
// 			)
// 		});
// 	}

// 	group.finish();
// }

// criterion_group!(
// 	name = benches;
// 	config = Criterion::default();
// 	targets = bench
// );

// criterion_main!(benches);
