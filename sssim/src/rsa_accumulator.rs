use crate::multiset::MultiSet;
use lazy_static::lazy_static;
use rug::{ops::Pow, Integer};
use serde::Serialize;

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

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
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

type MembershipWitness = Integer;
type NonMembershipWitness = (Integer, Integer);
pub type Witness = (MembershipWitness, NonMembershipWitness);

impl RsaAccumulatorDigest {
    #[must_use]
    pub fn verify(&self, member: &Integer, revision: u32, witness: Witness) -> bool {
        // member@revision is valid IF member@revision is in the set and
        // member@(revision+1) is not.
        self.verify_member(member, revision, witness.0)
            && self.verify_nonmember(member, revision + 1, witness.1)
    }

    #[must_use]
    fn verify_member(&self, member: &Integer, revision: u32, witness: Integer) -> bool {
        let exponent = member.pow(&revision.into());
        witness
            .pow_mod(&exponent.into(), &MODULUS)
            .expect("Non negative member")
            == self.value
    }

    #[allow(non_snake_case)]
    #[must_use]
    fn verify_nonmember(
        &self,
        member: &Integer,
        revision: u32,
        witness: NonMembershipWitness,
    ) -> bool {
        //TODO clean up clones
        let (a, B) = witness;
        let temp1 = self
            .value
            .clone()
            .pow_mod(&a, &MODULUS)
            .expect("Non negative witness");
        let exponent = member.pow(&revision.into());
        let temp2 = B
            .pow_mod(&exponent.into(), &MODULUS)
            .expect("Non negative value");
        (temp1 * temp2) % MODULUS.clone() == GENERATOR.clone()
    }

    #[must_use]
    pub fn verify_append_only(&self, proof: &Integer, new_state: Self) -> bool {
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
    #[must_use]
    pub fn digest(&self) -> &RsaAccumulatorDigest {
        &self.digest
    }

    pub fn increment(&mut self, member: Integer) {
        assert!(member >= 0);
        self.digest
            .value
            .pow_mod_mut(&member, &MODULUS)
            .expect("member should be >=0");
        self.multiset.insert(member);
    }

    #[must_use]
    pub fn prove_append_only(&self, other: &Self) -> Integer {
        assert!(self.multiset.is_superset(&other.multiset));
        let mut prod: Integer = 1.into();
        //TODO not this
        for (elem, count) in self.multiset.difference(&other.multiset) {
            prod *= Integer::from(elem.pow(count));
        }
        prod
    }

    pub fn prove(&self, member: &Integer, revision: u32) -> Option<Witness> {
        self.prove_member(member, revision).and_then(|mem_pf| {
            self.prove_nonmember(member, revision)
                .map(|nonmem_pf| (mem_pf, nonmem_pf))
        })
    }

    pub fn get(&self, member: &Integer) -> u32 {
        self.multiset.get(member)
    }

    //TODO compute all proofs?
    #[must_use]
    fn prove_member(&self, member: &Integer, revision: u32) -> Option<MembershipWitness> {
        assert!(member >= &0);
        let count = self.multiset.get(member);
        if count < revision {
            return None;
        }
        let mut current = GENERATOR.clone();
        for (s, count) in self.multiset.iter() {
            if s != member {
                let exp = Integer::from(s.pow(count));
                current.pow_mod_mut(&exp, &MODULUS).expect("member > 0");
            }
        }
        Some(current)
    }

    #[must_use]
    fn prove_nonmember(&self, value: &Integer, revision: u32) -> Option<NonMembershipWitness> {
        let count = self.multiset.get(&value);
        if count >= revision {
            return None;
        }

        let mut exp = Integer::from(1);
        //TODO not this
        for (s, count) in self.multiset.iter() {
            exp *= Integer::from(s.pow(count));
        }

        let (g, s, t) = exp.gcd_cofactors(value.pow(revision).into(), Integer::new());

        if g != 1 {
            return None;
        }

        let x = GENERATOR.clone();

        Some((s, x.pow_mod(&t, &MODULUS).unwrap()))
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
            assert_eq!(acc.prove(&value), None);

            acc.add(value.clone());

            let witness = acc.prove(&value).unwrap();
            assert!(acc.digest.verify(&value, witness));
        }
    }

    #[test]
    fn test_rsa_accumulator() {
        let acc = RsaAccumulator::default();
        assert_eq!(acc.digest.value, GENERATOR.clone());
    }

    #[test]
    fn test_rsa_accumulator_nonmember() {
        let mut acc = RsaAccumulator::default();
        let witness = acc.prove_nonmember(5.into()).unwrap();
        assert!(acc.digest.verify_nonmember(&5.into(), witness));

        acc.add(5.into());
        assert_eq!(acc.prove_nonmember(5.into()), None);
    }
}
