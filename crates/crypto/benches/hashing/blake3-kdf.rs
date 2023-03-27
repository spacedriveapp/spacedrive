use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use sd_crypto::{
	hashing::Hasher,
	types::{DerivationContext, Key, Salt},
};

const CONTEXT: DerivationContext =
	DerivationContext::new("crypto 2023-03-21 11:31:38 benchmark testing context");

fn bench(c: &mut Criterion) {
	let key = Key::generate();
	let salt = Salt::generate();
	c.bench_function("blake3-kdf", |b| {
		b.iter_batched(
			|| (key.clone(), salt),
			|(key, salt)| Hasher::derive_key(key, salt, CONTEXT),
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
