#![feature(
    nonzero_ops,
    iter_array_chunks,
    trait_alias,
    type_changing_struct_update
)]
#![allow(dead_code)]
pub mod accumulator;
pub mod authenticator;
mod bit_twiddling;
pub mod hash_to_prime;
pub mod log;
pub mod multiset;
mod poke;
pub mod primitives;
pub mod simulator;
pub mod util;

pub use authenticator::{Authenticator, BatchAuthenticator, PoolAuthenticator};
