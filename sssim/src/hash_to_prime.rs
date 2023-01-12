use digest::{ExtendableOutput, Update, XofReader};
use rug;
use sha3::{Sha3XofReader, Shake256};

use crate::primitives::Prime;
use thiserror::Error;

pub struct IntegerHasher {
    reader: Sha3XofReader,
    result: Vec<u8>,
}

impl IntegerHasher {
    pub fn new(data: &[u8], digits: usize) -> Self {
        // Here, we use Shake256 which is an "extendable output function" (XOF).
        // This is basically a hash function that gives you as many bytes of output
        // as you want. We need a weird number of bytes which depends on `digits`,
        // *and* we may need to try many times in a row, so the XOF gives us as much
        // hash data as we need.
        let mut hasher = Shake256::default();
        hasher.update(data);
        let reader = hasher.finalize_xof();
        let result: Vec<u8> = vec![0; digits];
        Self { reader, result }
    }

    pub fn hash(&mut self) -> rug::Integer {
        self.reader.read(&mut self.result);
        rug::Integer::from_digits(&self.result, rug::integer::Order::Lsf)
    }
}

#[derive(Error, Debug)]
pub enum HashToPrimeError {
    #[error("too many iters")]
    TooManyIters,
}

/// Hash the value of data to a 256-bit prime number.
pub fn hash_to_prime(data: &[u8]) -> Result<Prime, HashToPrimeError> {
    // We want a random number with a number of bits just greater than modulus
    // has. significant_digits gives us the right number of bytes.
    let digits: usize = 32;
    let mut bar = IntegerHasher::new(data, digits);

    // TODO(maybe): calculate how many times we should actually do this.
    // It appears to be between 10,000 and 100,000.
    for _ in 0..10000 {
        if let Ok(prime) = Prime::try_from(bar.hash()) {
            return Ok(prime);
        }
    }
    Err(HashToPrimeError::TooManyIters)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_hash_to_prime(data: Vec<u8>) {
            let result: Prime = hash_to_prime(&data)?;
            prop_assert!(result.significant_bits() <= 256);
        }

        #[test]
        fn test_hash_to_prime_unique(data1: Vec<u8>, data2: Vec<u8>) {
            prop_assume!(data1 != data2);
            prop_assert_ne!(hash_to_prime(&data1), hash_to_prime(&data2));
        }
    }
}
