#![allow(dead_code)]
use crate::poke;
use crate::{multiset::MultiSet, util::DataSizeFromSerialize};
use lazy_static::lazy_static;
use rayon::prelude::*;
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
    fn update(&mut self, value: Integer, new_element: Integer, digest: Integer) {
        // check that c^a * d^x = g
        debug_assert_eq!(
            (digest.clone().pow_mod(&self.exp, &MODULUS).unwrap()
                * self.base.clone().pow_mod(&value, &MODULUS).unwrap())
                % MODULUS.clone(),
            GENERATOR.clone(),
            "precondition",
        );

        // If we're adding another copy of *this* value to the accumulator, no update is necessary (the proof is still against the same digest as before!).
        if value == new_element {
            return;
        }

        // new_exp * member + _ * value = 1
        let (gcd, s, t) =
            Integer::gcd_cofactors(value.clone(), new_element.clone(), Integer::new());
        debug_assert_eq!(gcd, 1u8);

        let (q, r) = (self.exp.clone() * t).div_rem(value.clone());
        let new_exp = r;

        let mut new_base = self.base.clone();
        new_base *= digest
            .clone()
            .pow_mod(&(q * new_element.clone() + self.exp.clone() * s), &MODULUS)
            .unwrap();

        let c_hat = digest
            .clone()
            .pow_mod(&new_element, &MODULUS)
            .expect(">= 0");
        debug_assert_eq!(
            (c_hat.pow_mod(&new_exp, &MODULUS).unwrap()
                * new_base.clone().pow_mod(&value, &MODULUS).unwrap())
                % MODULUS.clone(),
            GENERATOR.clone()
        );

        self.exp = new_exp;
        self.base = new_base;
    }
}

#[derive(Clone, Serialize, Debug)]
pub struct Witness {
    member: Option<MembershipWitness>,
    nonmember: NonMembershipWitness,
}

impl DataSizeFromSerialize for Witness {}

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
    type AppendOnlyWitness = (Vec<poke::Proof>, Vec<Integer>);

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
        let mut cur = self.value.clone();
        for (inner_proof, value) in Iterator::zip(proof.0.iter(), proof.1.iter()) {
            let zku = poke::ZKUniverse {
                modulus: &MODULUS,
                lambda: 256,
            };
            let instance = poke::Instance {
                w: value.clone(),
                u: cur,
            };
            if !zku.verify(instance, inner_proof.clone()) {
                return false;
            }
            cur = value.clone();
        }
        cur == new_state.value
    }
}

impl DataSizeFromSerialize for HashMap<Integer, Witness> {}

impl BatchDigest for RsaAccumulatorDigest {
    type BatchWitness = HashMap<Integer, <Self as Digest>::Witness>;

    fn verify_batch(
        &self,
        members: &HashMap<Integer, u32>,
        mut witness: Self::BatchWitness,
    ) -> bool {
        // TODO: do better
        for (member, revision) in members {
            let proof = match witness.remove(member) {
                Some(proof) => proof,
                None => {
                    return false; // missing proof
                }
            };
            if !self.verify(member, *revision, proof) {
                return false;
            }
        }
        return true;
    }
}

impl BatchAccumulator for RsaAccumulator {
    fn prove_batch(
        &mut self,
        entries: &HashMap<Integer, u32>,
    ) -> <<Self as Accumulator>::Digest as BatchDigest>::BatchWitness {
        // TODO: do better
        let mut proofs = HashMap::default();
        for (member, revision) in entries {
            let proof = self.prove(member, *revision).unwrap();
            proofs.insert(member.clone(), proof);
        }
        proofs
    }
}

#[derive(Default, Debug, Clone, Serialize)]
struct SkipListEntry {
    //list of proofs from node n
    proofs: Vec<poke::Proof>,
}

impl SkipListEntry {
    fn add(&mut self, new: poke::Proof) {
        self.proofs.push(new);
    }

    fn find_next(&self, offset: usize) -> (poke::Proof, usize) {
        for (i, proof) in self.proofs.iter().enumerate().rev() {
            if 1 << i <= offset {
                return (proof.clone(), 1 << i);
            }
        }
        // not found
        panic!("offset too big")
    }
}

#[derive(Default, Debug, Clone, Serialize)]
pub struct RsaAccumulator {
    digest: RsaAccumulatorDigest,
    multiset: MultiSet<Integer>,
    proof_cache: HashMap<Integer, Witness>,
    digest_history: Vec<RsaAccumulatorDigest>,
    increment_history: Vec<Integer>,
    // append_only_proofs: Vec<Option<AppendOnlyWitness>>,
    append_only_proofs: Vec<SkipListEntry>,
    digests_to_indexes: HashMap<RsaAccumulatorDigest, usize>,
}

static PRECOMPUTE_CHUNK_SIZE: usize = 4;

fn precompute_helper(
    values: &[Integer],
    counts: &[u32],
    proof: NonMembershipWitness,
    g: Integer,
) -> Vec<Witness> {
    debug_assert_eq!(values.len(), counts.len());
    if values.len() == 0 {
        panic!("slice len should not be 0");
    }
    if values.len() == 1 {
        debug_assert!(
            RsaAccumulatorDigest::from(g.clone()).verify_nonmember(&values[0], proof.clone())
        );
        return vec![Witness {
            member: Some(MembershipWitness(g)),
            nonmember: proof,
        }];
    }
    let split_idx = values.len() / 2;
    let mut values_left = Integer::from(1u8);
    let mut values_right = Integer::from(1u8);
    let mut values_star = Integer::from(1u8);
    let mut members_left = Integer::from(1u8);
    let mut members_right = Integer::from(1u8);
    let mut members_star = Integer::from(1u8);
    let mut g_left = g.clone();
    let mut g_right = g.clone();
    let mut g_star = g.clone();
    for idx in 0..values.len() {
        let value = values[idx].clone();
        let count = counts[idx];
        let member = value.clone().pow(count);
        values_star *= value.clone();
        g_star.pow_mod_mut(&member, &MODULUS).expect(">= 0");
        members_star *= member.clone();
        if idx < split_idx {
            values_left *= value;
            g_left.pow_mod_mut(&member, &MODULUS).expect(">= 0");
            members_left *= member;
        } else {
            values_right *= value;
            g_right.pow_mod_mut(&member, &MODULUS).expect(">= 0");
            members_right *= member;
        }
    }
    debug_assert!(RsaAccumulatorDigest::from(g_star.clone()).verify_member(
        &members_star,
        1,
        MembershipWitness(g.clone())
    ));
    debug_assert!(
        RsaAccumulatorDigest::from(g.clone()).verify_nonmember(&values_star, proof.clone())
    );

    // s * e_l + t * e_r = 1
    // => a = a * s * e_l + a * t * e_r
    let (gcd, s, t) =
        Integer::gcd_cofactors(values_left.clone(), members_right.clone(), Integer::new());
    debug_assert_eq!(gcd, 1u8);
    let at = proof.exp.clone() * t.clone();
    // reduce a * t mod e_left
    let (q, r) = at.clone().div_rem(values_left.clone());
    let mut b_left = g
        .clone()
        .pow_mod(&(q * members_right.clone()), &MODULUS)
        .expect(">= 0");
    b_left *= g
        .clone()
        .pow_mod(&(proof.exp.clone() * s.clone()), &MODULUS)
        .expect(">= 0");
    b_left *= proof
        .base
        .clone()
        .pow_mod(&(values_right.clone()), &MODULUS)
        .expect(">= 0");
    let proof_left = NonMembershipWitness {
        exp: r,
        base: b_left,
    };

    // symmetric
    let (gcd, s, t) =
        Integer::gcd_cofactors(values_right.clone(), members_left.clone(), Integer::new());
    debug_assert_eq!(gcd, 1u8);
    let at = proof.exp.clone() * t.clone();
    // reduce a * t mod e_right
    let (q, r) = at.clone().div_rem(values_right.clone());
    let mut b_right = g
        .clone()
        .pow_mod(&(q * members_left.clone()), &MODULUS)
        .expect(">= 0");
    b_right *= g
        .clone()
        .pow_mod(&(proof.exp.clone() * s.clone()), &MODULUS)
        .expect(">= 0");
    b_right *= proof
        .base
        .clone()
        .pow_mod(&(values_left.clone()), &MODULUS)
        .expect(">= 0");
    let proof_right = NonMembershipWitness {
        exp: r,
        base: b_right,
    };

    debug_assert!(RsaAccumulatorDigest::from(g_left.clone()).verify_member(
        &members_left,
        1,
        MembershipWitness(g.clone())
    ));
    debug_assert!(RsaAccumulatorDigest::from(g_right.clone()).verify_member(
        &members_right,
        1,
        MembershipWitness(g.clone())
    ));
    debug_assert!(RsaAccumulatorDigest::from(g_right.clone())
        .verify_nonmember(&values_left, proof_left.clone()));
    debug_assert!(RsaAccumulatorDigest::from(g_left.clone())
        .verify_nonmember(&values_right, proof_right.clone()));

    debug_assert_eq!(
        (&values[..split_idx]).len() + (&values[split_idx..]).len(),
        values.len()
    );
    let (mut ret, r_ret) = if values.len() >= PRECOMPUTE_CHUNK_SIZE {
        rayon::join(
            || {
                precompute_helper(
                    &values[..split_idx],
                    &counts[..split_idx],
                    proof_left,
                    g_right,
                )
            },
            || {
                precompute_helper(
                    &values[split_idx..],
                    &counts[split_idx..],
                    proof_right,
                    g_left,
                )
            },
        )
    } else {
        (
            precompute_helper(
                &values[..split_idx],
                &counts[..split_idx],
                proof_left,
                g_right,
            ),
            precompute_helper(
                &values[split_idx..],
                &counts[split_idx..],
                proof_right,
                g_left,
            ),
        )
    };
    ret.extend_from_slice(&r_ret);
    ret
}

fn precompute(values: &[Integer], counts: &[u32]) -> Vec<Witness> {
    let mut e_star = Integer::from(1u8);
    for (value, count) in std::iter::zip(values.iter(), counts) {
        e_star *= value.clone().pow(count);
    }

    // a * 1 + b * e_star = 1
    let (gcd, a, b) = Integer::gcd_cofactors(1u8.into(), e_star.clone(), Integer::new());
    debug_assert_eq!(gcd, 1u8);
    let mut base = GENERATOR.clone();
    base.pow_mod_mut(&b, &MODULUS).expect(">= 0");
    let proof = NonMembershipWitness { exp: a, base };

    precompute_helper(values, counts, proof, GENERATOR.clone())
}

impl RsaAccumulator {
    //TODO compute all proofs?
    #[must_use]
    fn prove_member(&self, member: &Integer, revision: u32) -> Option<MembershipWitness> {
        debug_assert!(member >= &0);
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

        let mut exp = Integer::from(1u8);
        //TODO not this
        for (s, count) in self.multiset.iter() {
            exp *= Integer::from(s.pow(count));
        }

        // Bezout coefficients:
        // gcd = exp * s + value * t = 1
        let (gcd, s, t) = Integer::gcd_cofactors(exp, value.into(), Integer::new());

        if gcd != 1u8 {
            unreachable!("value should be coprime with the exponent of the accumulator");
        }
        // TODO: fix the size of exp (t)

        let d = GENERATOR.clone().pow_mod(&t, &MODULUS).unwrap();

        debug_assert_eq!(
            (self.digest.value.clone().pow_mod(&s, &MODULUS).unwrap()
                * d.clone().pow_mod(&value, &MODULUS).unwrap())
                % MODULUS.clone(),
            GENERATOR.clone(),
            "initially generating nonmembership proof failed"
        );

        Some(NonMembershipWitness { exp: s, base: d })
    }
}

fn find_max_pow(mut index: usize) -> usize {
    let mut max_pow = 0;
    while index % 2 == 0 {
        max_pow += 1;
        index = index >> 1;
    }
    max_pow
}

impl Accumulator for RsaAccumulator {
    type Digest = RsaAccumulatorDigest;
    #[must_use]
    fn digest(&self) -> &RsaAccumulatorDigest {
        &self.digest
    }

    /// TODO: implement better `increment_batch`

    /// O(N)
    fn increment(&mut self, member: Integer) {
        debug_assert!(member >= 0u8);

        // We need to update every membership proof, *except* our own!
        self.proof_cache.par_iter_mut().for_each(|(value, proof)| {
            let digest = proof.member.clone().unwrap().0.clone();
            proof
                .nonmember
                .update(value.clone(), member.clone(), digest);
            if value == &member {
                return;
            }
            proof.member.as_mut().unwrap().update(&member);
        });

        // If this is the first time this value was added, create a new membership proof.
        //
        // Because the membership proof is just the digest *without* the member
        // added, this is just the digest *before* we add the member!
        if self.proof_cache.get_mut(&member).is_none() {
            let membership_proof = MembershipWitness(self.digest.value.clone());
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
        self.multiset.insert(member.clone());

        self.increment_history.push(member.clone());
        // Add a *slot* for the new append-only proof.
        self.append_only_proofs.push(SkipListEntry::default());
        assert_eq!(self.increment_history.len(), self.append_only_proofs.len());
        assert_eq!(self.increment_history.len(), self.digest_history.len());

        let mut exponent = Integer::from(1u8);
        let index = self.increment_history.len();
        let max_pow = find_max_pow(index);

        for (member, proof_slot, old_digest, cur_index) in itertools::izip!(
            self.increment_history.iter().rev(),
            self.append_only_proofs.iter_mut().rev(),
            self.digest_history.iter().rev(),
            0..(1 << max_pow)
        ) {
            exponent *= member;
            let instance = poke::Instance {
                w: self.digest.value.clone(),
                u: old_digest.value.clone(),
            };
            let zku = poke::ZKUniverse {
                modulus: &MODULUS,
                lambda: 256,
            };

            //check if cur_index is a power of 2 or is equal to 1
            // (power of 2 - 1 is all 1s)
            if cur_index != 0 && cur_index & (cur_index - 1) == 0 {
                proof_slot.add(zku.prove(
                    instance,
                    poke::Witness {
                        x: exponent.clone(),
                    },
                ))
            }
        }

        // Update the digest history.
        self.digest_history.push(self.digest.clone());
        self.digests_to_indexes
            .insert(self.digest.clone(), self.digest_history.len() - 1);
    }

    #[must_use]
    fn prove_append_only(
        &self,
        prefix: &Self::Digest,
    ) -> <Self::Digest as Digest>::AppendOnlyWitness {
        if &self.digest == prefix {
            panic!("identical");
        }
        let idx = self.digests_to_indexes.get(prefix).unwrap();
        let mut cur_idx = 0;
        let mut proof_list = vec![];
        let mut value_list = vec![];

        while cur_idx < *idx {
            let (proof, offset) = self.append_only_proofs[cur_idx].find_next(idx - cur_idx);
            cur_idx += offset;
            proof_list.push(proof);
            value_list.push(self.digest_history[cur_idx].value.clone())
        }

        (proof_list, value_list)
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

    fn import(multiset: MultiSet<Integer>) -> Self {
        // Precompute membership proofs:
        let mut values = Vec::<Integer>::with_capacity(multiset.len());
        let mut counts = Vec::<u32>::with_capacity(multiset.len());
        let mut digest = GENERATOR.clone(); // TODO: repeat less work
        for (value, count) in multiset.iter() {
            // let exp = Integer::from(value.pow(count));
            for _ in 0..*count {
                digest.pow_mod_mut(value, &MODULUS).unwrap();
            }
            values.push(value.clone());
            counts.push(*count);
        }
        let mut proofs = precompute(&values, &counts);

        let mut proof_cache: HashMap<Integer, Witness> = Default::default();
        for _ in 0..values.len() {
            let value = values.pop().unwrap();
            let witness = proofs.pop().unwrap();
            proof_cache.insert(value.clone(), witness);
        }

        let digest = RsaAccumulatorDigest::from(digest);
        let mut digests_to_indexes: HashMap<RsaAccumulatorDigest, usize> = Default::default();
        digests_to_indexes.insert(digest.clone(), 0);
        Self {
            digest: digest.clone(),
            multiset,
            proof_cache,
            digest_history: vec![digest],
            append_only_proofs: vec![],
            increment_history: vec![],
            digests_to_indexes,
        }
    }
}

#[cfg(test)]
use proptest::prelude::*;

use super::{BatchAccumulator, BatchDigest};

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
        fn test_rsa_accumulator(mut acc: RsaAccumulator, value1 in primes(), value2 in primes()) {
            prop_assume!(value1 != value2);
            acc.increment(value2.clone());
            for rev in 0..10 {
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

                // increment value1
                acc.increment(value1.clone());

                // check value2
                prop_assert!(acc.prove(&value2, 2).is_none());
                let witness = acc.prove(&value2, 1).expect("should be able to prove current revision");
                prop_assert!(acc.digest.verify(&value2, 1, witness));
            }
        }

        #[test]
        fn test_rsa_accumulator_precompute(values in prop::collection::vec(primes(), 1..50)) {
            // let values: Vec<_> = values.into_iter().collect();
            let multiset = MultiSet::<Integer>::from(values);
            println!("new test, {:?}, {:?}", multiset.len(), multiset.inner);
            let mut acc = RsaAccumulator::import(multiset.clone());
            println!("verify");
            for (value, count) in multiset.iter() {
                println!("value: {:?}", value);
                prop_assert!(acc.prove(&value, count - 1).is_none());
                prop_assert!(acc.prove(&value, count + 1).is_none());
                let witness = acc.prove(&value, *count).expect("should be able to prove current revision");
                prop_assert!(acc.digest.verify(&value, *count, witness));
            }
            println!("increment and verify");
            for (value, count) in multiset.iter() {
                acc.increment(value.clone());
                prop_assert!(acc.prove(&value, *count).is_none());
                prop_assert!(acc.prove(&value, *count + 2).is_none());
                let witness = acc.prove(&value, count + 1).expect("should be able to prove current revision");
                prop_assert!(acc.digest.verify(&value, count + 1, witness));
            }
        }

    }

    #[test]
    fn test_rsa_accumulator_default() {
        let acc = RsaAccumulator::default();
        assert_eq!(acc.digest.value, GENERATOR.clone());
    }
}
