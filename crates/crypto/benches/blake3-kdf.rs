use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use sd_crypto::{
	primitives::CRYPTO_TEST_CONTEXT,
	types::{Key, Salt},
};

fn bench(c: &mut Criterion) {
	let key = Key::generate();
	let salt = Salt::generate();
	c.bench_function("blake3-kdf", |b| {
		b.iter_batched(
			|| (key.clone(), salt),
			|(key, salt)| Key::derive(key, salt, CRYPTO_TEST_CONTEXT),
			BatchSize::LargeInput,
		)
	});
}

criterion_group!(
	name = benches;
	config = Criterion::default();
	targets = bench
);

criterion_main!(benches);
