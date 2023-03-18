use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use sd_crypto::{
	crypto::{Decryptor, Encryptor},
	types::{Algorithm, Key, Nonce},
};

const ALGORITHM: Algorithm = Algorithm::Aes256Gcm;

const KB: usize = 1024;

const SIZES: [usize; 8] = [
	KB * 16,
	KB * 32,
	KB * 64,
	KB * 128,
	KB * 512,
	KB * 1024,
	KB * 2048,
	KB * 4096,
];

fn bench(c: &mut Criterion) {
	let mut group = c.benchmark_group("aes-256-gcm");

	for size in SIZES {
		let buf = vec![0u8; size].into_boxed_slice();

		let key = Key::generate();
		let nonce = Nonce::generate(ALGORITHM);

		let encrypted_bytes =
			Encryptor::encrypt_bytes(key.clone(), nonce, ALGORITHM, &buf, &[]).unwrap(); // bytes to decrypt

		group.throughput(criterion::Throughput::Bytes(size as u64));

		group.bench_function(BenchmarkId::new("encrypt", size), |b| {
			b.iter_batched(
				|| (key.clone(), nonce),
				|(key, nonce)| Encryptor::encrypt_bytes(key, nonce, ALGORITHM, &buf, &[]).unwrap(),
				BatchSize::SmallInput,
			)
		});

		group.bench_function(BenchmarkId::new("decrypt", size), |b| {
			b.iter_batched(
				|| (key.clone(), nonce),
				|(key, nonce)| {
					Decryptor::decrypt_bytes(key, nonce, ALGORITHM, &encrypted_bytes, &[]).unwrap()
				},
				BatchSize::SmallInput,
			)
		});
	}

	group.finish();
}

criterion_group!(
	name = benches;
	config = Criterion::default();
	targets = bench
);

criterion_main!(benches);
