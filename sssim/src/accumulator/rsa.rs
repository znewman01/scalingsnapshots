use crate::multiset::MultiSet;
use lazy_static::lazy_static;
use rug::{ops::Pow, Integer};
use serde::Serialize;
use std::collections::HashMap;

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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RsaAccumulatorDigest {
    value: Integer,
}

impl Serialize for RsaAccumulatorDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.value.to_string())
    }
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

#[derive(Clone, Serialize, Debug)]
struct MembershipWitness(Integer);

impl MembershipWitness {
    fn update(&mut self, value: &Integer) {
        self.0.pow_mod_mut(value, &MODULUS).expect("member >= 0");
    }
}

#[derive(Clone, Serialize, Debug)]
struct NonMembershipWitness {
    exp: Integer,
    base: Integer,
}

impl NonMembershipWitness {
    fn update(&mut self, value: Integer, member: Integer, digest: Integer) {
        // TODO: special case for value == member?
        //
        // new_exp * member + _ * value = 1
        let (gcd, mut new_exp, _) =
            Integer::gcd_cofactors(member.clone(), value.clone(), Integer::new());
        assert_eq!(gcd, 1u8);

        new_exp *= self.exp.clone();
        new_exp %= value.clone();

        // ahat * xhat = a + r * x  (note: ahat is smallish)
        // TODO: replace with div_exact
        let (r, rem) = (new_exp.clone() * member.clone() - self.exp.clone()).div_rem(value.clone());
        assert_eq!(rem, 0u8);
        let mut new_base = self.base.clone();
        new_base *= digest.clone().pow_mod(&r, &MODULUS).expect("r >= 0");
        new_base %= MODULUS.clone();

        self.exp = new_exp;
        self.base = new_base;
    }
}

#[derive(Clone, Serialize, Debug)]
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
        let exponent = member.pow(&revision);
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
                println!(
                    "verify member: {}",
                    self.verify_member(member, revision, mem_pf.clone())
                );
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
    proof_cache: HashMap<Integer, Witness>,
}

fn precompute_helper(slice: &[Integer], proof: NonMembershipWitness, g: Integer) -> Vec<Witness> {
    if slice.len() == 0 {
        panic!("slice len should not be 0");
    }
    if slice.len() <= 1 {
        return vec![Witness {
            member: Some(MembershipWitness(g)),
            nonmember: proof,
        }];
    }
    let split_idx = slice.len() / 2;
    let mut e_left = Integer::from(1u8);
    let mut e_right = Integer::from(1u8);
    let mut g_left = g.clone();
    let mut g_right = g.clone();
    for (idx, e) in slice.iter().enumerate() {
        if idx <= split_idx {
            e_left *= e.clone();
            g_left.pow_mod_mut(e, &MODULUS).expect(">= 0");
        } else {
            e_right *= e.clone();
            g_right.pow_mod_mut(e, &MODULUS).expect(">= 0");
        }
    }

    // a = a * s * e_l + a * t * e_r
    let (gcd, s, t) = Integer::gcd_cofactors(e_left.clone(), e_right.clone(), Integer::new());
    assert_eq!(gcd, 1u8);

    // reduce a * t mod e_left
    let (q, r) = (proof.exp.clone() * t.clone()).div_rem(e_left.clone());
    let a_left = r;
    let mut b_left = g
        .clone()
        .pow_mod(&(q * e_right.clone()), &MODULUS)
        .expect(">= 0");
    b_left *= g
        .clone()
        .pow_mod(&(proof.exp.clone() * s.clone()), &MODULUS)
        .expect(">= 0");
    b_left *= proof
        .base
        .clone()
        .pow_mod(&e_right, &MODULUS)
        .expect(">= 0");
    let proof_left = NonMembershipWitness {
        exp: a_left,
        base: b_left,
    };

    // do the same for a_R and b_R
    let (q, r) = (proof.exp.clone() * t).div_rem(e_right.clone());
    let a_right = r;
    let mut b_right = g
        .clone()
        .pow_mod(&(q * e_left.clone()), &MODULUS)
        .expect(">= 0");
    b_right *= g.clone().pow_mod(&(proof.exp * s), &MODULUS).expect(">= 0");
    b_right *= proof.base.pow_mod(&e_left, &MODULUS).expect(">= 0");
    let proof_right = NonMembershipWitness {
        exp: a_right,
        base: b_right,
    };

    let mut l_ret = precompute_helper(&slice[..split_idx], proof_left, g_right);
    let r_ret = precompute_helper(&slice[split_idx..], proof_right, g_left);
    l_ret.extend_from_slice(&r_ret);
    l_ret
}

fn precompute(slice: &[Integer]) -> Vec<Witness> {
    let mut e_star = Integer::from(1u8);
    for e in slice.iter() {
        e_star *= e.clone();
    }

    // a * 1 + b * e_star = 1
    let (gcd, a, b) = Integer::gcd_cofactors(GENERATOR.clone(), e_star.clone(), Integer::new());
    assert_eq!(gcd, 1u8);
    let mut base = GENERATOR.clone();
    base.pow_mod_mut(&b, &MODULUS).expect(">= 0");
    let proof = NonMembershipWitness { exp: a, base };

    precompute_helper(slice, proof, GENERATOR.clone())
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
        if self.multiset.get(value) != 0 {
            return None; // value is a member!
        }

        let mut exp = Integer::from(1);
        //TODO not this
        for (s, count) in self.multiset.iter() {
            exp *= Integer::from(s.pow(count));
        }

        // Bezout coefficients:
        // gcd = exp * s + value * t = 1
        let (gcd, s, t) = Integer::gcd_cofactors(exp, value.into(), Integer::new());

        if gcd != 1 {
            unreachable!("value should be coprime with the exponent of the accumulator");
        }

        Some(NonMembershipWitness {
            exp: s,
            base: GENERATOR.clone().pow_mod(&t, &MODULUS).unwrap(),
        })
    }
}

impl Accumulator for RsaAccumulator {
    type Digest = RsaAccumulatorDigest;
    #[must_use]
    fn digest(&self) -> &RsaAccumulatorDigest {
        &self.digest
    }

    // TODO precompute nonmembership proofs
    fn precompute_proofs(&mut self) {
        // Precompute membership proofs:
        let mut members = Vec::with_capacity(self.multiset.len());
        let mut values = Vec::with_capacity(self.multiset.len());
        for (s, count) in self.multiset.iter() {
            let exp = Integer::from(s.pow(count));
            values.push(exp);
            members.push(s);
        }
        let mut proofs = precompute(&values);

        for _ in 0..members.len() {
            let member = members.pop().unwrap();
            let witness = proofs.pop().unwrap();
            self.proof_cache.insert(member.clone(), witness);
        }
    }

    /// O(N)
    fn increment(&mut self, member: Integer) {
        assert!(member >= 0u8);

        // We need to update every membership proof, *except* our own!
        for (value, proof) in self.proof_cache.iter_mut() {
            proof
                .nonmember
                .update(value.clone(), member.clone(), self.digest.value.clone());
            if value == &member {
                continue;
            }
            proof.member.as_mut().unwrap().update(value);
        }

        // If this is the first time this value was added, create a new membership proof.
        //
        // Because the membership proof is just the digest *without* the member
        // added, this is just the digest *before* we add the member!
        let membership_proof = MembershipWitness(self.digest.value.clone());
        if self.proof_cache.get_mut(&member).is_none() {
            let proof = Witness {
                member: Some(membership_proof),
                nonmember: self.prove_nonmember(&member).unwrap(),
            };
            self.proof_cache.insert(member.clone(), proof);
        }

        // Update the digest to add the member.
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

    fn prove(&mut self, member: &Integer, revision: u32) -> Option<Witness> {
        if self.multiset.get(member) != revision {
            return None;
        }
        if revision == 0 {
            return self.prove_nonmember(member).map(Witness::for_zero);
        }
        self.proof_cache.get(member).cloned()
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
        prop_oneof![
            Just(2.into()),
            Just(3.into()),
            Just(5.into()),
            Just(7.into()),
            Just(11.into()),
            Just(13.into()),
        ]
    }

    proptest! {
        #[test]
        fn test_rsa_accumulator_inner(mut acc: RsaAccumulator, value1 in primes(), value2 in primes()) {
            println!("start new test");
            prop_assume!(value1 != value2);
                acc.increment(value2.clone());
            for rev in 0..10 {
                println!("rev: {}", rev);
                // At the start of this loop, we have exactly `rev` copies of `value` accumulated.
                if rev > 0 {
                    prop_assert!(acc.prove(&value1, rev - 1).is_none());
                    // prop_assert!(acc.prove(&value2, rev - 1).is_none());
                }
                prop_assert!(acc.prove(&value2, 2).is_none());
                let witness = acc.prove(&value2, 1).expect("should be able to prove current revision");
                prop_assert!(acc.digest.verify(&value2, 1, witness));
                // check value1
                prop_assert!(acc.prove(&value1, rev + 1).is_none());
                let witness = acc.prove(&value1, rev).expect("should be able to prove current revision");
                prop_assert!(acc.digest.verify(&value1, rev, witness));
                acc.increment(value1.clone());
                // check value2
                prop_assert!(acc.prove(&value2, 2).is_none());
                let witness = acc.prove(&value2, 1).expect("should be able to prove current revision");
                prop_assert!(acc.digest.verify(&value2, 1, witness));
            }
        }
    }

    #[test]
    fn test_rsa_accumulator() {
        let acc = RsaAccumulator::default();
        assert_eq!(acc.digest.value, GENERATOR.clone());
    }
}
