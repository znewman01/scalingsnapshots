use sha3::{Shake256};
use digest::{ExtendableOutput, Update, XofReader};
use rug;

fn hash_to_prime(data:&[u8], modulus: &rug::Integer) -> rug::Integer {
    let mut hasher = Shake256::default();

    hasher.update(data);

    let mut reader = hasher.finalize_xof();
    let mut result: Vec<u8> = vec![0;modulus.significant_digits::<u8>()];
    loop{
        reader.read(&mut result);
        let candidate = rug::Integer::from_digits(&result, rug::integer::Order::Lsf);
        if &candidate > modulus {
            continue;
        }
        if candidate.is_probably_prime(30) != rug::integer::IsPrime::No{
            return candidate;
        }
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn test_fail(){
        let data = [1u8;16];
        let mut modulus: rug::Integer = u64::MAX.into();
        let result = hash_to_prime(&data, &modulus);
        assert_ne!(result,0);
    }
}
