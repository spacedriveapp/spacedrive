use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use sd_crypto::{
	hashing::Hasher,
	primitives::{BLOCK_LEN, KEY_LEN},
};

const SIZES: [usize; 2] = [KEY_LEN, BLOCK_LEN];

fn bench(c: &mut Criterion) {
	let mut group = c.benchmark_group("blake3");

	for size in SIZES {
		let buf = vec![0u8; size].into_boxed_slice();

		group.throughput(Throughput::Bytes(size as u64));

		group.bench_function(BenchmarkId::new("hash", size), |b| {
			b.iter(|| Hasher::blake3(&buf))
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
