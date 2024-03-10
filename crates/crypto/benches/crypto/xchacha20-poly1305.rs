use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use sd_crypto::{
	crypto::{Decryptor, Encryptor},
	primitives::{BLOCK_LEN, KEY_LEN},
	types::{Aad, Algorithm, Key, Nonce},
};

const ALGORITHM: Algorithm = Algorithm::XChaCha20Poly1305;
const SIZES: [usize; 3] = [BLOCK_LEN, BLOCK_LEN * 2, BLOCK_LEN * 4];

fn bench(c: &mut Criterion) {
	let mut group = c.benchmark_group(ALGORITHM.to_string().to_ascii_lowercase());

	let key = Key::generate();
	let nonce = Nonce::generate(ALGORITHM);

	{
		group.throughput(Throughput::Bytes(KEY_LEN as u64));

		let test_key = Key::generate();
		let test_key_encrypted =
			Encryptor::encrypt_key(&key, &nonce, ALGORITHM, &test_key, Aad::Null).unwrap();

		group.bench_function(BenchmarkId::new("encrypt", "key"), |b| {
			b.iter(|| {
				Encryptor::encrypt_key(&key, &nonce, ALGORITHM, &test_key, Aad::Null).unwrap()
			});
		});

		group.bench_function(BenchmarkId::new("decrypt", "key"), |b| {
			b.iter(|| {
				Decryptor::decrypt_key(&key, ALGORITHM, &test_key_encrypted, Aad::Null).unwrap()
			});
		});
	}

	for size in SIZES {
		group.throughput(Throughput::Bytes(size as u64));

		let buf = vec![0u8; size].into_boxed_slice();

		let encrypted_bytes =
			Encryptor::encrypt_bytes(&key, &nonce, ALGORITHM, &buf, Aad::Null).unwrap(); // bytes to decrypt

		group.bench_function(BenchmarkId::new("encrypt", size), |b| {
			b.iter(|| Encryptor::encrypt_bytes(&key, &nonce, ALGORITHM, &buf, Aad::Null).unwrap());
		});

		group.bench_function(BenchmarkId::new("decrypt", size), |b| {
			b.iter(|| {
				Decryptor::decrypt_bytes(&key, &nonce, ALGORITHM, &encrypted_bytes, Aad::Null)
					.unwrap()
			})
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
