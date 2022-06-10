use digest::{ExtendableOutput, Update, XofReader};
use rug;
use rug::Complete;
use sha3::{Sha3XofReader, Shake256};
use std::convert::TryInto;

#[derive(Debug)]
pub struct MyError;

impl std::fmt::Display for MyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MyError")
    }
}

impl std::error::Error for MyError {}

struct IntegerHasher {
    reader: Sha3XofReader,
    result: Vec<u8>,
}

impl IntegerHasher {
    fn new(data: &[u8], digits: usize) -> Self {
        // Here, we use Shake256 which is an "extendable output function" (XOF).
        // This is basically a hash function that gives you as many bytes of output
        // as you want. We need a weird number of bytes which depends on modulus,
        // *and* we may need to try many times in a row, so the XOF gives us as much
        // hash data as we need.
        let mut hasher = Shake256::default();
        hasher.update(data);
        let reader = hasher.finalize_xof();
        let result: Vec<u8> = vec![0; digits];
        Self { reader, result }
    }

    fn hash(&mut self) -> rug::Integer {
        self.reader.read(&mut self.result);
        rug::Integer::from_digits(&self.result, rug::integer::Order::Lsf)
    }
}

struct RandMod<'a, F> {
    rand: F,
    modulus: &'a rug::Integer,
    bits: u32,
    t: rug::Integer,
}

impl<'a, F> RandMod<'a, F>
where
    F: FnMut() -> rug::Integer,
{
    fn new(rand: F, modulus: &'a rug::Integer) -> Self {
        const BITS_PER_BYTE: usize = 8;
        let bits: u32 = (modulus.significant_digits::<u8>() * BITS_PER_BYTE)
            .try_into()
            .unwrap();
        // declarations for the loop
        let (t, _) = ((rug::Integer::from(2) << bits) - modulus)
            .div_rem_ref(modulus)
            .complete();
        Self {
            rand,
            modulus,
            bits,
            t,
        }
    }

    // https://arxiv.org/abs/1805.10941
    fn rand_mod(&mut self) -> rug::Integer {
        let mut m = (self.rand)() * self.modulus;
        let mut l = m.clone().keep_bits(self.bits);
        if &l < self.modulus {
            while l < self.t {
                m = (self.rand)() * self.modulus;
                l = m.clone().keep_bits(self.bits);
            }
        }

        let candidate = m >> self.bits;
        assert!(&candidate < self.modulus);
        candidate
    }
}

#[must_use]
pub fn division_intractable_hash(data: &[u8], modulus: &rug::Integer) -> rug::Integer {
    // We want a random number with a number of bits just greater than modulus
    // has. significant_digits gives us the right number of bytes.
    let digits: usize = modulus.significant_digits::<u8>();
    let mut bar = IntegerHasher::new(data, digits);
    RandMod::new(|| bar.hash(), modulus).rand_mod()
}

// TODO: hash to 256-bit (security parameter) prime

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn integers() -> impl Strategy<Value = rug::Integer> {
        any::<u128>().prop_map(rug::Integer::from)
    }

    proptest! {
        #[test]
        fn test_division_intractable_hash(data: Vec<u8>, modulus in integers()) {
            prop_assume!(modulus > 128);
            let result = division_intractable_hash(&data, &modulus);
            prop_assert!(result < modulus);
        }
    }
}
