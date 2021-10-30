use digest::{ExtendableOutput, Update, XofReader};
use rug;
use sha3::Shake256;

// How sure do we want to be that our primes are actually prime?
// We want to be 30 sure.
const MILLER_RABIN_ITERS: u32 = 30;

#[derive(Debug)]
pub struct MyError;

impl std::fmt::Display for MyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        panic!();
        // f.write("MyError");
    }
}

impl std::error::Error for MyError {}

/// Hash the value of data to a prime number less than modulus.
pub fn hash_to_prime(data: &[u8], modulus: &rug::Integer) -> Result<rug::Integer, MyError> {
    // Here, we use Shake256 which is an "extendable output function" (XOF).
    // This is basically a hash function that gives you as many bytes of output
    // as you want. We need a weird number of bytes which depends on modulus,
    // *and* we may need to try many times in a row, so the XOF gives us as much
    // hash data as we need.
    let mut hasher = Shake256::default();

    hasher.update(data);

    let mut reader = hasher.finalize_xof();
    // We want a random number with a number of bits just greater than modulus
    // has. significant_digits gives us the right number of bytes.
    let mut result: Vec<u8> = vec![0; modulus.significant_digits::<u8>()];

    // TODO: calculate how many times we should actually do this.
    // It appears to be between 10,000 and 100,000.
    for _ in 0..100000 {
        reader.read(&mut result);
        let candidate = rug::Integer::from_digits(&result, rug::integer::Order::Lsf);
        // We want a number smaller than modulus (that's why it's our modulus).
        if &candidate > modulus {
            continue;
        }
        // Also it should be prime.
        if candidate.is_probably_prime(MILLER_RABIN_ITERS) == rug::integer::IsPrime::No {
            continue;
        }
        // If we made it here, our candidate rocks.
        return Ok(candidate);
    }
    Err(MyError)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn integers() -> impl Strategy<Value = rug::Integer> {
        any::<u128>().prop_map(rug::Integer::from)
    }

    proptest! {
        #[test]
        fn test_hash_to_prime(data: Vec<u8>, modulus in integers()) {
            prop_assume!(modulus > 128);
            let result = hash_to_prime(&data, &modulus)?;
            prop_assert!(result < modulus);
            prop_assert!(result.is_probably_prime(30) != rug::integer::IsPrime::No);
        }
    }
}
