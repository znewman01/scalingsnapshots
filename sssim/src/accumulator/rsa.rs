#![allow(dead_code)]
use crate::accumulator::{Accumulator, BatchAccumulator};
use crate::poke;
use crate::primitives::{Group, PositiveInteger, Prime};
use crate::util::DataSized;
use crate::{multiset::MultiSet, util::DataSizeFromSerialize};
use rayon::prelude::*;
use rug::Complete;
use rug::{ops::Pow, Integer};
use serde::Serialize;
use std::borrow::Borrow;
use std::collections::HashMap;
#[derive(Clone, Default, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct RsaAccumulatorDigest<G> {
    value: G,
}

impl<G> DataSized for RsaAccumulatorDigest<G> {
    fn size(&self) -> crate::util::Information {
        todo!()
    }
}

impl<G> From<G> for RsaAccumulatorDigest<G> {
    fn from(value: G) -> Self {
        RsaAccumulatorDigest { value }
    }
}

#[derive(Clone, Serialize, Debug)]
struct MembershipWitness<G>(G);

impl<G: Group> MembershipWitness<G> {
    fn update(&mut self, value: &Prime) {
        self.0 *= value.borrow()
    }
}

#[derive(Clone, Serialize, Debug)]
pub struct NonMembershipWitness<G> {
    exp: Integer,
    base: G,
}

impl<G: Group + 'static> NonMembershipWitness<G> {
    fn update(&mut self, value: &Prime, new_element: Prime, digest: G) {
        // check that c^a * d^x = g
        debug_assert_eq!(
            &((digest.clone() * &self.exp) + (self.base.clone() * value.inner())),
            G::one(),
            "precondition",
        );

        // If we're adding another copy of *this* value to the accumulator, no
        // update is necessary (the proof is still against the same digest as
        // before!).
        if value == &new_element {
            return;
        }

        // new_exp * member + _ * value = 1
        let (gcd, s, t) = Integer::gcd_cofactors_ref(value.inner(), new_element.borrow()).into();
        debug_assert_eq!(gcd, 1u8);

        let (q, r) = (self.exp.clone() * t).div_rem_ref(value.inner()).complete();
        let new_exp = r;

        let mut new_base = self.base.clone();
        let x: Integer = q * Into::<Integer>::into(new_element.clone()) + self.exp.clone() * s;
        new_base = new_base + digest.clone() * &x;

        let c_hat = digest.clone() * new_element.borrow();
        debug_assert_eq!(
            &(c_hat.clone() * &new_exp + new_base.clone() * value.inner()),
            G::one(),
        );

        self.exp = new_exp;
        self.base = new_base;
    }
}

#[derive(Clone, Serialize, Debug)]
pub struct Witness<G> {
    member: Option<MembershipWitness<G>>,
    nonmember: NonMembershipWitness<G>,
}

impl<G> DataSizeFromSerialize for Witness<G> where G: Serialize {}

impl<G> Witness<G> {
    fn new(member: MembershipWitness<G>, nonmember: NonMembershipWitness<G>) -> Self {
        Witness {
            member: Some(member),
            nonmember,
        }
    }

    fn for_zero(nonmember: NonMembershipWitness<G>) -> Self {
        Witness {
            member: None,
            nonmember,
        }
    }
}

impl<G: Group + 'static> RsaAccumulatorDigest<G> {
    fn verify_member(&self, member: &Prime, revision: u32, witness: MembershipWitness<G>) -> bool {
        let exponent =
            PositiveInteger::try_from(Into::<Integer>::into(member.clone()).pow(&revision))
                .expect("prime power");
        witness.0 * exponent.borrow() == self.value
    }

    #[allow(non_snake_case)]
    #[must_use]
    fn verify_nonmember(&self, member: &Prime, witness: NonMembershipWitness<G>) -> bool {
        // https://link.springer.com/content/pdf/10.1007/978-3-540-72738-5_17.pdf
        let l = self.value.clone() * &witness.exp;
        let r = witness.base * member.borrow();
        &(l + r) == G::one()
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct AppendOnlyWitness<G> {
    pokes: Vec<poke::Proof<G>>,
    values: Vec<G>,
}

impl<G> DataSized for AppendOnlyWitness<G> {
    fn size(&self) -> crate::util::Information {
        todo!()
    }
}

impl<G> DataSizeFromSerialize for HashMap<Prime, Witness<G>> where G: Serialize {}

impl<G: Group + TryFrom<Integer> + 'static> BatchAccumulator for RsaAccumulator<G> {
    type BatchDigest = RsaAccumulatorDigest<G>;
    type BatchWitness = HashMap<Prime, <Self as Accumulator>::Witness>;

    fn prove_batch<I: IntoIterator<Item = Prime>>(
        &mut self,
        entries: I,
    ) -> (HashMap<Prime, u32>, Self::BatchWitness) {
        // TODO: do better using BBF19
        let mut counts: HashMap<Prime, u32> = Default::default();
        let mut proofs: HashMap<Prime, Self::Witness> = Default::default();
        for member in entries {
            let revision = self.get(&member);
            let proof = self.prove(&member, revision).unwrap();
            counts.insert(member.clone(), revision);
            proofs.insert(member, proof);
        }
        (counts, proofs)
    }

    fn increment_batch<I: IntoIterator<Item = Prime>>(
        &mut self,
        members: I,
    ) -> Option<Self::AppendOnlyWitness> {
        let old_digest = self.digest.clone();

        // TODO: parallelize but that's tricky
        let mut exponent = Integer::from(1u8);
        for member in members {
            self.increment(member.clone());
            exponent *= Into::<Integer>::into(member);
        }

        let zku = poke::ZKUniverse::default();
        let instance = poke::Instance {
            w: old_digest.value,
            u: self.digest.value.clone(),
        };
        let witness = poke::Witness { x: exponent };
        let proof = zku.prove(instance, witness);
        Some(AppendOnlyWitness {
            pokes: vec![proof],
            values: vec![self.digest.value.clone()],
        })
    }

    fn verify_batch(
        digest: &Self::BatchDigest,
        members: &HashMap<Prime, u32>,
        mut witness: Self::BatchWitness,
    ) -> bool {
        // TODO: do better using BBF19?
        for (member, revision) in members {
            let proof = match witness.remove(member) {
                Some(proof) => proof,
                None => {
                    return false; // missing proof
                }
            };
            if !Self::verify(digest, member, *revision, proof) {
                return false;
            }
        }
        return true;
    }
}

#[derive(Default, Debug, Clone, Serialize)]
struct SkipListEntry<G> {
    //list of proofs from node n
    proofs: Vec<poke::Proof<G>>,
}

impl<G: Group> SkipListEntry<G> {
    fn add(&mut self, new: poke::Proof<G>) {
        self.proofs.push(new);
    }

    fn find_next(&self, offset: usize) -> (poke::Proof<G>, usize) {
        for (i, proof) in self.proofs.iter().enumerate().rev() {
            if 1 << i <= offset {
                return (proof.clone(), 1 << i);
            }
        }
        // not found
        panic!("offset too big")
    }
}

#[derive(Default, Debug, Clone)]
pub struct RsaAccumulator<G> {
    digest: RsaAccumulatorDigest<G>,
    multiset: MultiSet<Prime>,
    proof_cache: HashMap<Prime, Witness<G>>,
    nonmember_proof_cache: HashMap<Prime, NonMembershipWitness<G>>,
    digest_history: Vec<RsaAccumulatorDigest<G>>,
    increment_history: Vec<Prime>,
    append_only_proofs: Vec<SkipListEntry<G>>,
    digests_to_indexes: HashMap<RsaAccumulatorDigest<G>, usize>,
    exponent: Integer,
}

impl<G> DataSized for RsaAccumulator<G> {
    fn size(&self) -> crate::util::Information {
        todo!()
    }
}

static PRECOMPUTE_CHUNK_SIZE: usize = 4;

/// returns (Vec<Witness>, digest, exponent)
fn precompute_helper<G: Group + 'static>(
    values: &[Prime],
    counts: &[u32],
    proof: NonMembershipWitness<G>,
    g: G,
) -> (Vec<Witness<G>>, G, Integer) {
    debug_assert_eq!(values.len(), counts.len());
    if values.len() == 0 {
        panic!("slice len should not be 0");
    }
    if values.len() == 1 {
        debug_assert!(
            RsaAccumulatorDigest::from(g.clone()).verify_nonmember(&values[0], proof.clone())
        );
        let exponent = Integer::from(values[0].clone()) * counts[0];
        let digest = g.clone() * &exponent;
        return (
            vec![Witness {
                member: Some(MembershipWitness(g)),
                nonmember: proof,
            }],
            digest,
            exponent,
        );
    }
    let split_idx = values.len() / 2;
    let mut values_left = Integer::from(1u8);
    let mut values_right = Integer::from(1u8);
    let mut values_star: Option<Integer> = None;
    let mut members_left = Integer::from(1u8);
    let mut members_right = Integer::from(1u8);
    let mut members_star: Option<Integer> = None;
    let mut g_left = g.clone();
    let mut g_right = g.clone();
    let mut g_star: Option<G> = None;
    for idx in 0..values.len() {
        let value = values[idx].clone();
        let count = counts[idx];
        let member = Integer::from(value.clone()).pow(count);
        if idx < split_idx {
            g_left *= &member.clone();
            values_left *= Integer::from(value);
            members_left *= member;
        } else {
            g_right *= &member.clone();
            values_right *= Integer::from(value.clone());
            members_right *= member.clone();

            g_star
                .get_or_insert_with(|| g_left.clone())
                .mul_assign(&member.clone());
            *values_star.get_or_insert_with(|| values_left.clone()) *= Integer::from(value);
            *members_star.get_or_insert_with(|| members_left.clone()) *= member;
        }
    }
    let g_star = g_star.unwrap();
    let members_star = members_star.unwrap();
    let values_star = values_star.unwrap();
    debug_assert!(RsaAccumulatorDigest::from(g_star.clone()).verify_member(
        &Prime::new_unchecked(members_star.clone()),
        1,
        MembershipWitness(g.clone())
    ));
    debug_assert!(RsaAccumulatorDigest::from(g.clone())
        .verify_nonmember(&Prime::new_unchecked(values_star), proof.clone()));

    // s * e_l + t * e_r = 1
    // => a = a * s * e_l + a * t * e_r
    let (gcd, s, t) =
        Integer::gcd_cofactors(values_left.clone(), members_right.clone(), Integer::new());
    debug_assert_eq!(gcd, 1u8);
    let at = proof.exp.clone() * t.clone();
    // reduce a * t mod e_left
    let (q, r) = at.clone().div_rem(values_left.clone());
    let mut b_left = g.clone() * &(q * members_right.clone());
    b_left += g.clone() * &(proof.exp.clone() * s.clone());
    b_left += proof.base.clone() * &values_right;
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
    let mut b_right = g.clone() * &(q * members_left.clone());
    b_right += g.clone() * &(proof.exp.clone() * s.clone());
    b_right += proof.base.clone() * &values_left;
    let proof_right = NonMembershipWitness {
        exp: r,
        base: b_right,
    };

    debug_assert!(RsaAccumulatorDigest::from(g_left.clone()).verify_member(
        &Prime::new_unchecked(members_left),
        1,
        MembershipWitness(g.clone())
    ));
    debug_assert!(RsaAccumulatorDigest::from(g_right.clone()).verify_member(
        &Prime::new_unchecked(members_right),
        1,
        MembershipWitness(g.clone())
    ));
    debug_assert!(RsaAccumulatorDigest::from(g_right.clone())
        .verify_nonmember(&Prime::new_unchecked(values_left), proof_left.clone()));
    debug_assert!(RsaAccumulatorDigest::from(g_left.clone())
        .verify_nonmember(&Prime::new_unchecked(values_right), proof_right.clone()));

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
                .0
            },
            || {
                precompute_helper(
                    &values[split_idx..],
                    &counts[split_idx..],
                    proof_right,
                    g_left,
                )
                .0
            },
        )
    } else {
        (
            precompute_helper(
                &values[..split_idx],
                &counts[..split_idx],
                proof_left,
                g_right,
            )
            .0,
            precompute_helper(
                &values[split_idx..],
                &counts[split_idx..],
                proof_right,
                g_left,
            )
            .0,
        )
    };
    ret.extend_from_slice(&r_ret);
    (ret, g_star, members_star)
}

/// Returns (Vec<Witness>, digest, exponent)
fn precompute<G: Group + 'static>(
    values: &[Prime],
    counts: &[u32],
) -> (Vec<Witness<G>>, G, Integer) {
    let mut e_star = Integer::from(1u8);
    for (value, count) in std::iter::zip(values.iter(), counts) {
        e_star *= Integer::from(value.clone()).pow(count);
    }

    // a * 1 + b * e_star = 1
    let (gcd, a, b) = Integer::gcd_cofactors(1u8.into(), e_star.clone(), Integer::new());
    debug_assert_eq!(gcd, 1u8);
    let base = G::default() * &b;
    let proof = NonMembershipWitness { exp: a, base };

    precompute_helper(values, counts, proof, G::default())
}

impl<G: Group + 'static> RsaAccumulator<G> {
    #[must_use]
    fn prove_member(&self, member: &Prime, revision: u32) -> Option<MembershipWitness<G>> {
        debug_assert!(<Prime as Borrow<Integer>>::borrow(member) >= &0);
        if revision > self.multiset.get(member) {
            return None;
        }
        let mut res = G::default();
        for (s, count) in self.multiset.iter() {
            if s != member {
                res *= &Integer::from(s.clone()).pow(count);
            }
        }
        Some(MembershipWitness(res))
    }

    #[must_use]
    fn prove_nonmember_uncached(&self, value: &Prime) -> Option<NonMembershipWitness<G>> {
        // https://link.springer.com/content/pdf/10.1007/978-3-540-72738-5_17.pdf
        if self.multiset.get(value) != 0 {
            return None; // value is a member!
        }

        // Bezout coefficients:
        // gcd: exp * s + value * t = 1
        let (gcd, s, t) = Integer::gcd_cofactors_ref(&self.exponent, value.borrow()).into();
        if gcd != 1u8 {
            unreachable!("value should be coprime with the exponent of the accumulator");
        }
        debug_assert!(&s < value.inner()); // s should be small-ish

        let d = G::default() * &t;

        debug_assert_eq!(
            &((self.digest.value.clone() * &s) + (d.clone() * value.inner())),
            G::one(),
            "initially generating nonmembership proof failed"
        );

        Some(NonMembershipWitness { exp: s, base: d })
    }

    #[must_use]
    fn prove_nonmember(&mut self, value: &Prime) -> Option<NonMembershipWitness<G>> {
        if let Some(proof) = self.nonmember_proof_cache.get(value) {
            return Some(proof.clone());
        }
        self.prove_nonmember_uncached(value).map(|proof| {
            self.nonmember_proof_cache
                .insert(value.clone(), proof.clone());
            proof
        })
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

impl<G: Group + TryFrom<Integer> + 'static> Accumulator for RsaAccumulator<G> {
    type Digest = RsaAccumulatorDigest<G>;
    type Witness = Witness<G>;
    type AppendOnlyWitness = AppendOnlyWitness<G>;
    type NonMembershipWitness = NonMembershipWitness<G>;

    #[must_use]
    fn digest(&self) -> &Self::Digest {
        &self.digest
    }

    /// O(N)
    fn increment(&mut self, member: Prime) {
        debug_assert!(member.inner() >= &0u8);

        // We need to update every membership proof, *except* our own!
        self.proof_cache.par_iter_mut().for_each(|(value, proof)| {
            let digest = proof.member.clone().unwrap().0.clone();
            proof.nonmember.update(value, member.clone(), digest);
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
        self.digest.value *= member.borrow();
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
            exponent *= member.inner();
            let instance = poke::Instance {
                w: self.digest.value.clone(),
                u: old_digest.value.clone(),
            };
            let zku = poke::ZKUniverse::<G>::default();

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

        // Invalidate the nonmembership proof cache.
        self.nonmember_proof_cache = Default::default();
    }

    #[must_use]
    fn prove_append_only(&self, prefix: &Self::Digest) -> Self::AppendOnlyWitness {
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

        AppendOnlyWitness {
            pokes: proof_list,
            values: value_list,
        }
    }

    fn prove(&mut self, member: &Prime, revision: u32) -> Option<Witness<G>> {
        if self.multiset.get(member) != revision {
            return None;
        }
        if revision == 0 {
            return self.prove_nonmember(member).map(Witness::for_zero);
        }
        self.proof_cache.get(member).cloned()
    }

    fn get(&self, member: &Prime) -> u32 {
        self.multiset.get(member)
    }

    fn import(multiset: MultiSet<Prime>) -> Self {
        // Precompute membership proofs:
        let mut values = Vec::<Prime>::with_capacity(multiset.len());
        let mut counts = Vec::<u32>::with_capacity(multiset.len());
        for (value, count) in multiset.iter() {
            values.push(value.clone());
            counts.push(*count);
        }
        let (mut proofs, digest, exponent) = precompute(&values, &counts);

        let mut proof_cache: HashMap<Prime, Witness<G>> = Default::default();
        for _ in 0..values.len() {
            let value = values.pop().unwrap();
            let witness = proofs.pop().unwrap();
            proof_cache.insert(value.clone(), witness);
        }

        let digest = RsaAccumulatorDigest::from(digest);
        let mut digests_to_indexes: HashMap<RsaAccumulatorDigest<G>, usize> = Default::default();
        digests_to_indexes.insert(digest.clone(), 0);
        Self {
            digest: digest.clone(),
            multiset,
            proof_cache,
            nonmember_proof_cache: Default::default(),
            digest_history: vec![digest],
            append_only_proofs: vec![],
            increment_history: vec![],
            digests_to_indexes,
            exponent,
        }
    }

    #[must_use]
    fn verify(
        digest: &Self::Digest,
        member: &Prime,
        revision: u32,
        witness: Self::Witness,
    ) -> bool {
        // member@revision is valid IF
        // (a) member@revision is in the set and
        // (b) member is NOT in the set corresponding to the membership proof for (a)

        match witness.member {
            Some(mem_pf) => {
                digest.verify_member(member, revision, mem_pf.clone())
                    && RsaAccumulatorDigest::from(mem_pf.0)
                        .verify_nonmember(member, witness.nonmember)
            }
            None => {
                // Special-case: revision = 0 has no membership proof.
                revision == 0 && digest.verify_nonmember(member, witness.nonmember)
            }
        }
    }

    #[must_use]
    fn verify_append_only(
        digest: &Self::Digest,
        proof: &Self::AppendOnlyWitness,
        new_state: &Self::Digest,
    ) -> bool {
        let mut cur = digest.value.clone();
        for (inner_proof, value) in Iterator::zip(proof.pokes.iter(), proof.values.iter()) {
            let zku = poke::ZKUniverse::<G>::default();
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

#[cfg(test)]
use proptest::prelude::*;

#[cfg(test)]
impl<G> Arbitrary for RsaAccumulator<G> {
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
