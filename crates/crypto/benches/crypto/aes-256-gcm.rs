use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use sd_crypto::{
	crypto::{Decryptor, Encryptor},
	primitives::{BLOCK_LEN, KEY_LEN},
	types::{Aad, Algorithm, Key, Nonce},
};

const ALGORITHM: Algorithm = Algorithm::Aes256Gcm;
const SIZES: [usize; 1] = [BLOCK_LEN];

fn bench(c: &mut Criterion) {
	let mut group = c.benchmark_group(ALGORITHM.to_string().to_ascii_lowercase());

	let key = Key::generate();
	let nonce = Nonce::generate(ALGORITHM);

	{
		group.throughput(Throughput::Bytes(KEY_LEN as u64));

		let test_key = Key::generate();
		let test_key_encrypted =
			Encryptor::encrypt_key(key.clone(), nonce, ALGORITHM, test_key.clone(), Aad::Null)
				.unwrap();

		group.bench_function(BenchmarkId::new("encrypt", "key"), |b| {
			b.iter_batched(
				|| (key.clone(), nonce, test_key.clone()),
				|(key, nonce, test_key)| {
					Encryptor::encrypt_key(key, nonce, ALGORITHM, test_key, Aad::Null).unwrap()
				},
				BatchSize::LargeInput,
			)
		});

		group.bench_function(BenchmarkId::new("decrypt", "key"), |b| {
			b.iter_batched(
				|| (key.clone(), nonce, test_key_encrypted),
				|(key, nonce, test_key_encrypted)| {
					Decryptor::decrypt_key(key, nonce, ALGORITHM, test_key_encrypted, Aad::Null)
						.unwrap()
				},
				BatchSize::LargeInput,
			)
		});
	}

	for size in SIZES {
		group.throughput(Throughput::Bytes(size as u64));

		let buf = vec![0u8; size].into_boxed_slice();

		let encrypted_bytes =
			Encryptor::encrypt_bytes(key.clone(), nonce, ALGORITHM, &buf, Aad::Null).unwrap(); // bytes to decrypt

		group.bench_function(BenchmarkId::new("encrypt", size), |b| {
			b.iter_batched(
				|| (key.clone(), nonce),
				|(key, nonce)| {
					Encryptor::encrypt_bytes(key, nonce, ALGORITHM, &buf, Aad::Null).unwrap()
				},
				BatchSize::LargeInput,
			)
		});

		group.bench_function(BenchmarkId::new("decrypt", size), |b| {
			b.iter_batched(
				|| (key.clone(), nonce),
				|(key, nonce)| {
					Decryptor::decrypt_bytes(key, nonce, ALGORITHM, &encrypted_bytes, Aad::Null)
						.unwrap()
				},
				BatchSize::LargeInput,
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
