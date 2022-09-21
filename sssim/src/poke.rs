//! Proof-of-knowledge of Exponentiation (PoKE) proofs.
//!
//! See [BBF18]: https://eprint.iacr.org/2018/1188.pdf
#![allow(non_snake_case)]
use crate::hash_to_prime::{hash_to_prime, IntegerHasher};
use rug::Integer;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Instance {
    w: Integer,
    u: Integer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Witness {
    x: Integer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Proof {
    z: Integer,
    Q: Integer,
    r: Integer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ZKUniverse {
    modulus: Integer,
    lambda: u64,
}

impl ZKUniverse {
    fn exp(&self, base: Integer, exp: &Integer) -> Integer {
        return base
            .pow_mod(exp, &self.modulus)
            .expect("Non-negative exponent");
    }

    fn fiat_shamir1(&self, instance: &Instance) -> Integer {
        let data_str = format!("{instance:?}");
        let bytes = self.modulus.significant_digits::<u8>();
        let mut hasher = IntegerHasher::new(data_str.as_bytes(), bytes);
        loop {
            // TODO: replace with fancier rejection sampling
            let value = hasher.hash();
            if value < self.modulus {
                return value;
            }
        }
    }

    fn fiat_shamir2(&self, instance: &Instance, g: &Integer, z: &Integer) -> Integer {
        let data_str = format!("{instance:?}{g:?}{z:?}");
        assert!(self.lambda == 256); // Assumed by `hash_to_prime`
        hash_to_prime(data_str.as_bytes()).unwrap()
    }

    fn fiat_shamir3(
        &self,
        instance: &Instance,
        g: &Integer,
        z: &Integer,
        ell: &Integer,
    ) -> Integer {
        let data_str = format!("{instance:?}{g:?}{z:?}{ell:?}");
        let mut hasher = IntegerHasher::new(data_str.as_bytes(), 32);
        hasher.hash()
    }

    fn prove(&self, instance: Instance, witness: Witness) -> Proof {
        let u = instance.u.clone();
        let w = instance.w.clone();
        let x = witness.x;
        assert!(u != 0);
        assert_eq!(self.exp(u.clone(), &x), w);

        // Verifier sends g <-$- G to the Prover
        let g: Integer = self.fiat_shamir1(&instance);

        // Prover sends z <- g^x \in G to the verifier.
        let z = self.exp(g.clone(), &x);

        // Verifier sends ell <-$- Primes(lambda) and alpha <-$- [0, 2^lambda).
        let ell = self.fiat_shamir2(&instance, &g, &z);
        let alpha = self.fiat_shamir3(&instance, &g, &z, &ell);

        // Prover finds the quotient q and residue r < ell such that x = ql + r.
        let (q, r) = x.div_rem(ell);

        // Prover sends Q = u^q g^(alpha q) and r to the Verifier
        let mut Q = self.exp(u, &q) * self.exp(g, &(alpha * q));
        Q %= &self.modulus;

        Proof { z, Q, r }
    }

    fn verify(&self, instance: Instance, proof: Proof) -> bool {
        let u = instance.u.clone();
        let w = instance.w.clone();
        let Q = proof.Q;
        let r = proof.r;
        let z = proof.z;

        for value in vec![&u, &w, &Q, &r, &z] {
            if value == &1 || value == &(self.modulus.clone() - 1) {
                return false;
            }
        }

        // From Fiat-Shamir
        let g = self.fiat_shamir1(&instance);
        let ell = self.fiat_shamir2(&instance, &g, &z);
        let alpha = self.fiat_shamir3(&instance, &g, &z, &ell);

        // Verifier accepts if r < ell and Q^ell u^r g^(alpha r) = w z^(alpha).
        let mut lhs = self.exp(Q, &ell) * self.exp(u, &r);
        lhs *= self.exp(g, &(alpha.clone() * r.clone()));
        let mut rhs = w * self.exp(z, &alpha);
        lhs = lhs % &self.modulus;
        rhs = rhs % &self.modulus;
        r < ell && lhs == rhs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn primes() -> impl Strategy<Value = Integer> {
        prop_oneof![
            Just(
                Integer::parse("37975227936943673922808872755445627854565536638199")
                    .unwrap()
                    .into()
            ),
            Just(
                Integer::parse("40094690950920881030683735292761468389214899724061")
                    .unwrap()
                    .into()
            ),
        ]
    }

    fn integers() -> impl Strategy<Value = Integer> {
        any::<u16>().prop_map(Integer::from)
    }

    fn int_mod(modulus: Integer) -> impl Strategy<Value = Integer> {
        integers().prop_map(move |value| value % modulus.clone())
    }

    #[derive(Debug, PartialEq, Eq)]
    struct PokeProblem {
        instance: Instance,
        witness: Witness,
        universe: ZKUniverse,
    }

    use proptest_derive::Arbitrary;
    #[derive(Debug, Arbitrary)]
    enum PokePart {
        Instance,
        WitnessOrProof,
        Universe,
    }

    impl Arbitrary for PokeProblem {
        type Parameters = ();
        type Strategy = BoxedStrategy<Self>;

        fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
            (primes(), primes())
                .prop_filter_map("rsa modulus (not prime power!)", |(p, q)| {
                    if p == q {
                        return None;
                    }
                    let modulus = p * q;
                    let lambda = 256;
                    Some(ZKUniverse { modulus, lambda })
                })
                .prop_flat_map(|universe| {
                    (int_mod(universe.modulus.clone()), integers()).prop_filter_map(
                        "domain",
                        move |(u, x)| {
                            if u == 0u8 || u == 1u8 || u == universe.modulus.clone() - 1u8 {
                                return None;
                            }
                            // We require x > 2^(256 + 1).
                            let x = x + (Integer::from(2u8) << 256);
                            let w = u.clone().pow_mod(&x, &universe.modulus).unwrap();
                            let universe = universe.clone();
                            let instance = Instance { u, w };
                            let witness = Witness { x };
                            Some(PokeProblem {
                                universe,
                                instance,
                                witness,
                            })
                        },
                    )
                })
                .boxed()
        }
    }

    proptest! {
        #[test]
        fn test_good(problem: PokeProblem) {
            let universe = problem.universe;
            let instance = problem.instance;
            let witness = problem.witness;

            let proof = universe.prove(instance.clone(), witness);
            prop_assert!(universe.verify(instance, proof));
        }

        #[test]
        #[should_panic]
        fn test_prove_bad(problem: PokeProblem, other: PokeProblem, part: PokePart) {
            let mut universe = problem.universe;
            let mut instance = problem.instance;
            let mut witness = problem.witness;

            match part {
                PokePart::WitnessOrProof => {
                    prop_assume!(witness != other.witness);
                    witness = other.witness;
                },
                PokePart::Instance => {
                    prop_assume!(instance != other.instance);
                    instance = other.instance;
                },
                PokePart::Universe => {
                    prop_assume!(universe != other.universe);
                    universe = other.universe;
                }
            };
            universe.prove(instance.clone(), witness);
        }

        fn test_verify_bad(problem: PokeProblem, other: PokeProblem, part: PokePart) {
            let mut universe = problem.universe;
            let mut instance = problem.instance;
            let witness = problem.witness;

            let mut proof = universe.prove(instance.clone(), witness);

            match part {
                PokePart::WitnessOrProof => {
                    proof.Q += 1;
                },
                PokePart::Instance => {
                    prop_assume!(instance != other.instance);
                    instance = other.instance;
                },
                PokePart::Universe => {
                    prop_assume!(universe != other.universe);
                    universe = other.universe;
                }
            };

            prop_assert!(!universe.verify(instance, proof));
        }
    }
}
