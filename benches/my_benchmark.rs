use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, BenchmarkId};
// use mycrate::fibonacci;
use rug::Integer;
use sssim::hash_to_prime::hash_to_prime;
use sssim::rsa_accumulator::{RsaAccumulator, RsaAccumulatorDigest, MODULUS};
use std::convert::TryInto;

pub fn criterion_benchmark(c: &mut Criterion) {
    // Make accumulator with one item
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

    // create an accumulator with N items
    //make primes
    static SIZES:&[usize] = &[1, 100];

    let mut group = c.benchmark_group("from_elem");
    group.sample_size(10);
    for s in SIZES.iter() {
        group.bench_with_input(BenchmarkId::from_parameter(s), &s, |b, s|{
            b.iter_batched(
                || {
                    (0..**s).into_iter().map(|x|{
                        hash_to_prime(&[x.try_into().unwrap()], &MODULUS).unwrap()
                    }).collect::<Vec<_>>()
                },
                |primes| RsaAccumulator::new(primes),
                BatchSize::LargeInput,
            )
        });
    }
    group.finish();

    //fix with https://arxiv.org/pdf/1805.10941.pdf
    c.bench_function("hash_to_prime 1", |b| {
        b.iter(||hash_to_prime(black_box(&[]), &MODULUS))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

// - hash_to_prime
//
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
