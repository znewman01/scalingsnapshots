//! Proof-of-knowledge of Exponentiation (PoKE) proofs.
//!
//! See [BBF18]: https://eprint.iacr.org/2018/1188.pdf
#![allow(non_snake_case)]
use std::marker::PhantomData;

use crate::hash_to_prime::{hash_to_prime, IntegerHasher};
use crate::primitives::{Group, Prime};
use rug::Integer;
use serde::Serialize;

use crate::util::{DataSized, Information};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance<G> {
    // new value
    pub w: G,
    // base
    pub u: G,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Witness {
    pub x: Integer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Proof<G> {
    z: G,
    Q: G,
    r: Integer,
}

impl<G> DataSized for Proof<G>
where
    G: DataSized,
{
    fn size(&self) -> Information {
        self.z.size() + self.Q.size() + self.r.size()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZKUniverse<G> {
    pub lambda: u64,
    _group: PhantomData<G>,
}

impl<G> Default for ZKUniverse<G> {
    fn default() -> Self {
        Self {
            lambda: 256,
            _group: Default::default(),
        }
    }
}

impl<G: Group + TryFrom<Integer> + 'static> ZKUniverse<G> {
    fn fiat_shamir1(&self, instance: &Instance<G>) -> G {
        let data_str = format!("{instance:?}");
        let bytes = G::bytes();
        let mut hasher = IntegerHasher::new(data_str.as_bytes(), bytes);
        loop {
            // TODO(maybe): replace with fancier rejection sampling
            if let Ok(value) = G::try_from(hasher.hash()) {
                return value;
            }
        }
    }

    fn fiat_shamir2(&self, instance: &Instance<G>, g: &G, z: &G) -> Prime {
        let data_str = format!("{instance:?}{g:?}{z:?}");
        hash_to_prime(data_str.as_bytes()).unwrap()
    }

    fn fiat_shamir3(&self, instance: &Instance<G>, g: &G, z: &G, ell: &Prime) -> Integer {
        let data_str = format!("{instance:?}{g:?}{z:?}{ell:?}");
        let mut hasher = IntegerHasher::new(data_str.as_bytes(), 32);
        hasher.hash()
    }

    pub fn prove(&self, instance: Instance<G>, witness: Witness) -> Proof<G> {
        let u = instance.u.clone();
        let w = instance.w.clone();
        let x = witness.x;
        assert_eq!(u.clone() * &x, w);

        // Verifier sends g <-$- G to the Prover
        let g = self.fiat_shamir1(&instance);

        // Prover sends z <- g^x \in G to the verifier.
        let z = g.clone() * &x;

        // Verifier sends ell <-$- Primes(lambda) and alpha <-$- [0, 2^lambda).
        let ell = self.fiat_shamir2(&instance, &g, &z);
        let alpha = self.fiat_shamir3(&instance, &g, &z, &ell);

        // Prover finds the quotient q and residue r < ell such that x = ql + r.
        let (q, r) = x.div_rem(ell.into_inner());

        // Prover sends Q = u^q g^(alpha q) and r to the Verifier
        let Q = u * &q + g * &(alpha * q);

        Proof { z, Q, r }
    }

    pub fn verify(&self, instance: Instance<G>, proof: Proof<G>) -> bool {
        let u = instance.u.clone();
        let w = instance.w.clone();
        let Q = proof.Q;
        let r = proof.r;
        let z = proof.z;

        // From Fiat-Shamir
        let g = self.fiat_shamir1(&instance);
        let ell = self.fiat_shamir2(&instance, &g, &z);
        let alpha = self.fiat_shamir3(&instance, &g, &z, &ell);

        // Verifier accepts if r < ell and Q^ell u^r g^(alpha r) = w z^(alpha).
        let lhs = Q * ell.as_ref() + u * &r + g * &(alpha.clone() * r.clone());
        let rhs = w + z * &alpha;
        &r < ell.inner() && lhs == rhs
    }
}

/*
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
    struct PokeProblem<G> {
        instance: Instance<G>,
        witness: Witness,
        universe: ZKUniverse<G>,
    }

    use proptest_derive::Arbitrary;
    #[derive(Debug, Arbitrary)]
    enum PokePart {
        Instance,
        WitnessOrProof,
        Universe,
    }

    impl<G> Arbitrary for PokeProblem<G>
    where
        PokeProblem<G>: std::fmt::Debug,
    {
        type Parameters = ();
        type Strategy = BoxedStrategy<Self>;

        fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
            Just(Some(ZKUniverse::default()))
                .prop_flat_map(|universe| {
                    (int_mod(universe.modulus.clone()), integers()).prop_filter_map(
                        "domain",
                        move |(u, x)| {
                            if u == 0u8 || u == 1u8 || u == universe.modulus.clone() - 1u8 {
                                return None;
                            }
                            // We require x > 2^(256 + 1).
                            let x = x + (Integer::from(2u8) << 256u32);
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
*/
