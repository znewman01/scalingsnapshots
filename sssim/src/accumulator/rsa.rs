use crate::multiset::MultiSet;
use lazy_static::lazy_static;
use rug::{ops::Pow, Integer};
use serde::Serialize;

use crate::accumulator::{Accumulator, Digest};

// RSA modulus from https://en.wikipedia.org/wiki/RSA_numbers#RSA-2048
// TODO generate a new modulus
lazy_static! {
    pub static ref MODULUS: Integer = Integer::parse(
        "2519590847565789349402718324004839857142928212620403202777713783604366202070\
           7595556264018525880784406918290641249515082189298559149176184502808489120072\
           8449926873928072877767359714183472702618963750149718246911650776133798590957\
           0009733045974880842840179742910064245869181719511874612151517265463228221686\
           9987549182422433637259085141865462043576798423387184774447920739934236584823\
           8242811981638150106748104516603773060562016196762561338441436038339044149526\
           3443219011465754445417842402092461651572335077870774981712577246796292638635\
           6373289912154831438167899885040445364023527381951378636564391212010397122822\
           120720357"
    )
    .unwrap()
    .into();
    static ref GENERATOR: Integer = Integer::from(65537);
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq, Hash)]
pub struct RsaAccumulatorDigest {
    value: Integer,
}

impl Default for RsaAccumulatorDigest {
    fn default() -> Self {
        RsaAccumulatorDigest {
            value: GENERATOR.clone(),
        }
    }
}
impl From<Integer> for RsaAccumulatorDigest {
    fn from(value: Integer) -> Self {
        RsaAccumulatorDigest { value }
    }
}

#[derive(Clone, Serialize)]
struct MembershipWitness(Integer);

#[derive(Clone, Serialize)]
struct NonMembershipWitness {
    exp: Integer,
    base: Integer,
}

#[derive(Clone, Serialize)]
pub struct Witness {
    member: Option<MembershipWitness>,
    nonmember: NonMembershipWitness,
}

impl Witness {
    fn new(member: MembershipWitness, nonmember: NonMembershipWitness) -> Self {
        Witness {
            member: Some(member),
            nonmember,
        }
    }

    fn for_zero(nonmember: NonMembershipWitness) -> Self {
        Witness {
            member: None,
            nonmember,
        }
    }
}

impl RsaAccumulatorDigest {
    fn verify_member(&self, member: &Integer, revision: u32, witness: MembershipWitness) -> bool {
        let exponent = member.pow(&revision.into());
        witness
            .0
            .pow_mod(&exponent.into(), &MODULUS)
            .expect("Non negative member")
            == self.value
    }

    #[allow(non_snake_case)]
    #[must_use]
    fn verify_nonmember(&self, member: &Integer, witness: NonMembershipWitness) -> bool {
        // https://link.springer.com/content/pdf/10.1007/978-3-540-72738-5_17.pdf
        // TODO: check size of member
        // TODO: check size of witness
        // TODO: clean up clones
        let temp1 = self
            .value
            .clone()
            .pow_mod(&witness.exp, &MODULUS)
            .expect("Non negative witness");
        let temp2 = witness
            .base
            .pow_mod(member, &MODULUS)
            .expect("Non negative value");
        (temp1 * temp2) % MODULUS.clone() == GENERATOR.clone()
    }
}

impl Digest for RsaAccumulatorDigest {
    type Witness = Witness;
    type AppendOnlyWitness = Integer;

    #[must_use]
    fn verify(&self, member: &Integer, revision: u32, witness: Self::Witness) -> bool {
        // member@revision is valid IF
        // (a) member@revision is in the set and
        // (b) member is NOT in the set corresponding to the membership proof for (a)

        match witness.member {
            Some(mem_pf) => {
                self.verify_member(member, revision, mem_pf.clone())
                    && RsaAccumulatorDigest::from(mem_pf.0)
                        .verify_nonmember(member, witness.nonmember)
            }
            None => {
                // Special-case: revision = 0 has no membership proof.
                revision == 0 && self.verify_nonmember(member, witness.nonmember)
            }
        }
    }

    #[must_use]
    fn verify_append_only(&self, proof: &Self::AppendOnlyWitness, new_state: &Self) -> bool {
        let expected = self
            .value
            .clone()
            .pow_mod(proof, &MODULUS)
            .expect("non-negative");
        expected == new_state.value
    }
}

#[derive(Default, Debug, Clone, Serialize)]
pub struct RsaAccumulator {
    digest: RsaAccumulatorDigest,
    multiset: MultiSet<Integer>,
}

impl RsaAccumulator {
    //TODO compute all proofs?
    #[must_use]
    fn prove_member(&self, member: &Integer, revision: u32) -> Option<MembershipWitness> {
        assert!(member >= &0);
        if revision > self.multiset.get(member) {
            return None;
        }
        let mut res = GENERATOR.clone();
        for (s, count) in self.multiset.iter() {
            if s != member {
                let exp = Integer::from(s.pow(count));
                res.pow_mod_mut(&exp, &MODULUS).expect("member > 0");
            }
        }
        Some(MembershipWitness(res))
    }

    #[must_use]
    fn prove_nonmember(&self, value: &Integer) -> Option<NonMembershipWitness> {
        // https://link.springer.com/content/pdf/10.1007/978-3-540-72738-5_17.pdf
        if self.multiset.get(&value) != 0 {
            return None; // value is a member!
        }

        let mut exp = Integer::from(1);
        //TODO not this
        for (s, count) in self.multiset.iter() {
            exp *= Integer::from(s.pow(count));
        }

        // Bezout coefficients:
        // gcd = exp * s + value * t
        let (gcd, s, t) = exp.gcd_cofactors(value.into(), Integer::new());

        if gcd != 1 {
            unreachable!("value should be coprime with the exponent of the accumulator");
        }

        let x = GENERATOR.clone();

        Some(NonMembershipWitness {
            exp: s,
            base: x.pow_mod(&t, &MODULUS).unwrap(),
        })
    }
}

impl Accumulator for RsaAccumulator {
    type Digest = RsaAccumulatorDigest;
    #[must_use]
    fn digest(&self) -> &RsaAccumulatorDigest {
        &self.digest
    }

    fn increment(&mut self, member: Integer) {
        assert!(member >= 0);
        self.digest
            .value
            .pow_mod_mut(&member, &MODULUS)
            .expect("member should be >=0");
        self.multiset.insert(member);
    }

    #[must_use]
    fn prove_append_only_from_vec(&self, other: &[Integer]) -> Integer {
        let mut prod: Integer = 1.into();
        // TODO: convert other into a multiset
        for elem in other {
            prod *= Integer::from(elem);
        }
        prod
    }

    #[must_use]
    fn prove_append_only(&self, other: &Self) -> Integer {
        assert!(self.multiset.is_superset(&other.multiset));
        let mut prod: Integer = 1.into();
        // TODO: not this
        for (elem, count) in self.multiset.difference(&other.multiset) {
            prod *= Integer::from(elem.pow(count));
        }
        prod
    }

    fn prove(&self, member: &Integer, revision: u32) -> Option<Witness> {
        if revision == 0 {
            return self.prove_nonmember(member).map(Witness::for_zero);
        }
        self.prove_member(member, revision).and_then(|mem_pf| {
            // TODO: more efficiently/better
            let mut ms = self.multiset.clone();
            for _ in 0..revision {
                ms.remove(member);
            }
            let acc = RsaAccumulator {
                digest: RsaAccumulatorDigest::from(mem_pf.0.clone()),
                multiset: ms,
            };
            acc.prove_nonmember(member)
                .map(|nonmem_pf| Witness::new(mem_pf, nonmem_pf))
        })
    }

    fn get(&self, member: &Integer) -> u32 {
        self.multiset.get(member)
    }
}

#[cfg(test)]
use proptest::prelude::*;

#[cfg(test)]
impl Arbitrary for RsaAccumulator {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;
    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        Just(RsaAccumulator::default()).boxed()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn primes() -> impl Strategy<Value = Integer> {
        Just(5.into())
    }

    proptest! {
        #[test]
        fn test_rsa_accumulator_inner(mut acc: RsaAccumulator, value in primes()) {
            for rev in 0..10 {
                if rev > 0 {
                    assert!(acc.prove(&value, rev - 1).is_none());
                }
                assert!(acc.prove(&value, rev + 1).is_none());
                let witness = acc.prove(&value, rev).expect("should be able to prove current revision");
                assert!(acc.digest.verify(&value, rev, witness));
                acc.increment(value.clone());
            }
        }
    }

    #[test]
    fn test_rsa_accumulator() {
        let acc = RsaAccumulator::default();
        assert_eq!(acc.digest.value, GENERATOR.clone());
    }
}
