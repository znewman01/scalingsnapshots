use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
// use mycrate::fibonacci;
use rug::Integer;
use sssim::hash_to_prime::hash_to_prime;
use sssim::rsa_accumulator::{RsaAccumulator, RsaAccumulatorDigest, MODULUS};

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("acc 1", |b| {
        b.iter_batched(
            || {
                (
                    RsaAccumulator::default(),
                    hash_to_prime(&[], &MODULUS).unwrap(),
                )
            },
            |(mut acc, value)| acc.add(black_box(value)),
            BatchSize::LargeInput,
        )
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

// - create accumulator with N items
//   1. make N random primes
//   2. (bench) make accumulator with N items
//
// - update accumulator
//   1. make N random primes
//   2. make accumulator with N items
//   3. (bench) update accumulator
//
// - compute membership proof
//   1. make accumulator with N items
//   2. (bench) compute membership proof (no cacheing)
//
// - compute nonmembership proof
//   1. make accumulator with N items
//   2. (bench) compute membership proof (no cacheing)
//
// - precompute proofs (when that's implemented)
//   1. make accumulator with N items
//   2. (bench) precompute all proofs (no cacheing)
//
// - fancier benchmark
//   1. make accumulator with N items
//   2. compute a proof
//   3. update the accumulator M times
//   4. (bench) recompute proof
