use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rug::Integer;
use sssim::accumulator::rsa::{multiply_stuff, multiply_stuff2};
use sssim::hash_to_prime::hash_to_prime;
use sssim::primitives::Prime;
use std::iter::successors;

pub fn criterion_benchmark(c: &mut Criterion) {
    let values: Vec<Integer> = (0..100u32)
        .into_iter()
        .map(|x| hash_to_prime(&format!("{x}").as_bytes()))
        .collect::<Result<Vec<Prime>, _>>()
        .unwrap()
        .into_iter()
        .map(Prime::into)
        .collect();

    let counts: Vec<u32> = successors(Some(1u32), |x| Some((x % 10) + 1))
        .take(values.len())
        .collect();

    let mut group = c.benchmark_group("sample-size-example");
    group.sample_size(10);
    group.bench_function("m1", |b| {
        b.iter(|| multiply_stuff(black_box(&values), black_box(&counts)))
    });
    group.bench_function("m2", |b| {
        b.iter(|| multiply_stuff2(black_box(&values), black_box(&counts)))
    });
    group.finish()
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
