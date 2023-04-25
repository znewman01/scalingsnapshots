#![allow(dead_code)]
use crate::accumulator::{Accumulator, BatchAccumulator};
use crate::poke;
use crate::primitives::{Collector, Group, PositiveInteger, Prime, SkipList};
use crate::util::byte;
use crate::util::DataSized;
use crate::{multiset::MultiSet, util::Information};
use rayon::prelude::*;
use rug::Complete;
use rug::{ops::Pow, Integer};
use serde::Serialize;
use std::borrow::Borrow;
use std::collections::HashMap;

use indicatif::ProgressBar;

#[derive(Clone, Default, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct RsaAccumulatorDigest<G> {
    value: G,
}

impl<G> DataSized for RsaAccumulatorDigest<G>
where
    G: DataSized,
{
    fn size(&self) -> crate::util::Information {
        self.value.size()
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

impl<G> DataSized for MembershipWitness<G>
where
    G: DataSized,
{
    fn size(&self) -> Information {
        self.0.size()
    }
}

#[derive(Clone, Serialize, Debug)]
pub struct NonMembershipWitness<G> {
    exp: Integer,
    base: G,
}

impl<G> DataSized for NonMembershipWitness<G>
where
    G: DataSized,
{
    fn size(&self) -> Information {
        self.exp.size() + self.base.size()
    }
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
        let (gcd, s, t) = Integer::extended_gcd_ref(value.inner(), new_element.borrow()).into();
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

impl<G> DataSized for Witness<G>
where
    MembershipWitness<G>: DataSized,
    NonMembershipWitness<G>: DataSized,
{
    fn size(&self) -> Information {
        self.member.size() + self.nonmember.size()
    }
}

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
    inner: Vec<(poke::Proof<G>, G)>,
}

impl<G> DataSized for AppendOnlyWitness<G>
where
    G: DataSized,
    poke::Proof<G>: DataSized,
{
    fn size(&self) -> crate::util::Information {
        let mut size = uom::ConstZero::ZERO;
        if self.inner.len() > 0 {
            size += u64::try_from(self.inner.len()).unwrap() * self.inner[0].0.size();
            size += u64::try_from(self.inner.len()).unwrap() * self.inner[0].1.size();
        }
        return size;
    }
}

#[derive(Clone, Serialize, Debug)]
pub struct BatchWitness<W> {
    inner: HashMap<Prime, W>,
}

impl<W: DataSized> DataSized for BatchWitness<W> {
    fn size(&self) -> Information {
        if self.inner.len() > 0 {
            let key = self.inner.keys().next().expect("hashmap not empty");
            let value = self.inner.get(key).expect("hashmap not empty");
            let len: u64 = self.inner.len().try_into().unwrap();
            return len * (key.size() + value.size());
        }
        return uom::ConstZero::ZERO;
    }
}

impl<G> DataSized for HashMap<Prime, Witness<G>>
where
    Witness<G>: DataSized,
{
    fn size(&self) -> Information {
        let len: u64 = self.len().try_into().unwrap();
        match self.iter().next() {
            None => Information::new::<byte>(0),
            Some((k, v)) => (k.size() + v.size()) * len,
        }
    }
}

impl<G: Group + TryFrom<Integer> + 'static> BatchAccumulator for RsaAccumulator<G>
where
    RsaAccumulator<G>: Accumulator<Digest = RsaAccumulatorDigest<G>>,
    BatchWitness<<Self as Accumulator>::Witness>: Clone,
{
    type BatchDigest = RsaAccumulatorDigest<G>;
    type BatchWitness = BatchWitness<Self::Witness>;

    fn prove_batch<I: IntoIterator<Item = Prime>>(
        &mut self,
        entries: I,
    ) -> (HashMap<Prime, u32>, Self::BatchWitness) {
        // TODO(maybe): do better using BBF19
        let mut counts: HashMap<Prime, u32> = Default::default();
        let mut proofs: HashMap<Prime, Self::Witness> = Default::default();
        for member in entries {
            let revision = self.get(&member);
            let proof = self.prove(&member, revision).unwrap();
            counts.insert(member.clone(), revision);
            proofs.insert(member, proof);
        }
        (counts, BatchWitness { inner: proofs })
    }

    fn increment_batch<I: IntoIterator<Item = Prime>>(
        &mut self,
        members: I,
    ) -> Option<Self::AppendOnlyWitness> {
        let old_digest = self.digest.clone();

        let mut exponent = Integer::from(1u8);

        let mut members_hashmap = HashMap::<Prime, u32>::default();
        for member in members {
            exponent *= member.inner().clone();
            *members_hashmap.entry(member).or_insert(0) += 1;
        }
        let exponent = Prime::new_unchecked(exponent);

        self.proof_cache.par_iter_mut().for_each(|(value, proof)| {
            let digest = proof.member.clone().unwrap().0;
            proof.nonmember.update(&value, exponent.clone(), digest);
            // for all members that equal value: divide by member
            let update_val = match members_hashmap.get(value) {
                Some(count) => Prime::new_unchecked(
                    exponent.clone().into_inner() / Integer::from(value.inner().pow(count)),
                ),
                None => exponent.clone(),
            };
            proof.member.as_mut().unwrap().update(&update_val);
        });

        // TODO(maybe): make n log n
        for (member, count) in &members_hashmap {
            if self.proof_cache.get_mut(member).is_none() {
                let exponent = exponent.inner().clone() / member.inner().pow(count).complete();
                let membership_proof = MembershipWitness(self.digest.value.clone() * &exponent);
                let mut nonmember_proof = self
                    .nonmember_proof_cache
                    .remove(member)
                    .unwrap_or_else(|| self.prove_nonmember_uncached(&member).unwrap());
                nonmember_proof.update(
                    &member,
                    Prime::new_unchecked(exponent),
                    membership_proof.clone().0.clone(),
                );
                let proof = Witness {
                    member: Some(membership_proof),
                    nonmember: nonmember_proof,
                };
                self.proof_cache.insert(member.clone(), proof);
            }
        }

        self.digest.value *= exponent.inner();
        self.exponent *= exponent.inner();

        for (member, count) in members_hashmap {
            for _ in 0..count {
                self.multiset.insert(member.clone());
            }
        }

        self.history.add(HistoryEntry {
            end_digest: self.digest.clone(),
            exponent: exponent.into(),
        });

        // Update the digest history.
        self.digests_to_indexes
            .insert(self.digest.clone(), self.history.len() - 1);

        // Invalidate the nonmembership proof cache.
        self.nonmember_proof_cache = Default::default();
        debug_assert_eq!(self.digest.value, G::one().clone() * &self.exponent);

        Some(self.prove_append_only(&old_digest))
    }

    fn verify_batch(
        digest: &Self::BatchDigest,
        members: &HashMap<Prime, u32>,
        mut witness: Self::BatchWitness,
    ) -> bool {
        // TODO(maybe): do better using BBF19?
        for (member, revision) in members {
            let proof = match witness.inner.remove(member) {
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

#[derive(Clone, Debug)]
pub struct HistoryEntry<G> {
    exponent: Integer,
    end_digest: RsaAccumulatorDigest<G>,
}

impl<G> Collector for HistoryEntry<G>
where
    G: Group + 'static,
    G: TryFrom<rug::Integer>,
{
    type Item = HistoryEntry<G>;
    type Proof = poke::Proof<G>;

    fn init(item: &Self::Item) -> Self {
        item.clone()
    }

    fn collect(&mut self, item: &Self::Item) {
        self.exponent *= item.exponent.clone();
    }

    fn to_proof(&self, item: &Self::Item) -> Self::Proof {
        let instance = poke::Instance {
            w: self.end_digest.value.clone(),
            u: item.end_digest.value.clone(),
        };
        let zku = poke::ZKUniverse::<G>::default();
        zku.prove(
            instance,
            poke::Witness {
                x: self.exponent.clone(),
            },
        )
    }
}

impl<G> DataSized for HistoryEntry<G>
where
    RsaAccumulatorDigest<G>: DataSized,
{
    fn size(&self) -> crate::util::Information {
        self.exponent.size() + self.end_digest.size()
    }
}

// TODO(probably not): shard storage across # cores
#[derive(Default, Debug, Clone)]
pub struct RsaAccumulator<G>
where
    HistoryEntry<G>: Collector,
    SkipList<HistoryEntry<G>>: std::fmt::Debug,
{
    digest: RsaAccumulatorDigest<G>,
    multiset: MultiSet<Prime>,
    proof_cache: HashMap<Prime, Witness<G>>,
    nonmember_proof_cache: HashMap<Prime, NonMembershipWitness<G>>,
    history: SkipList<HistoryEntry<G>>,
    digests_to_indexes: HashMap<RsaAccumulatorDigest<G>, usize>,
    exponent: Integer,
}

impl<G> DataSized for RsaAccumulator<G>
where
    HistoryEntry<G>: Collector,
    SkipList<HistoryEntry<G>>: std::fmt::Debug,
    SkipList<HistoryEntry<G>>: DataSized,
    RsaAccumulatorDigest<G>: DataSized,
    Witness<G>: DataSized,
    NonMembershipWitness<G>: DataSized,
{
    fn size(&self) -> crate::util::Information {
        let mut size = self.digest.size() + self.history.size() + self.exponent.size();
        let multi_len: u64 = self.multiset.len().try_into().unwrap();
        size += multi_len * Information::new::<byte>(4);
        if self.proof_cache.len() > 0 {
            let item = self.proof_cache.keys().next();
            let val = self.proof_cache.values().next();
            let len: u64 = self.proof_cache.len().try_into().unwrap();
            size +=
                (item.expect("map not empty").size() + val.expect("map not empty").size()) * len;
        }

        if self.nonmember_proof_cache.len() > 0 {
            let item = self.nonmember_proof_cache.keys().next();
            let val = self.nonmember_proof_cache.values().next();
            let len: u64 = self.nonmember_proof_cache.len().try_into().unwrap();
            size +=
                (item.expect("map not empty").size() + val.expect("map not empty").size()) * len;
        }

        if self.digests_to_indexes.len() > 0 {
            let item = self.digests_to_indexes.keys().next();
            let val = self.proof_cache.values().next();
            let len: u64 = self.digests_to_indexes.len().try_into().unwrap();
            size +=
                (item.expect("map not empty").size() + val.expect("map not empty").size()) * len;
        }
        size
    }
}

static PRECOMPUTE_CHUNK_SIZE: usize = 4;

/// returns (Vec<Witness>, digest, exponent)
fn precompute_helper<G: Group + 'static>(
    values: &[Prime],
    counts: &[u32],
    proof: NonMembershipWitness<G>,
    g: G,
    bar: &ProgressBar,
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
        Integer::extended_gcd(values_left.clone(), members_right.clone(), Integer::new());
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
        Integer::extended_gcd(values_right.clone(), members_left.clone(), Integer::new());
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

    bar.inc(values.len().try_into().unwrap());

    let (mut ret, r_ret) = if values.len() >= PRECOMPUTE_CHUNK_SIZE {
        rayon::join(
            || {
                precompute_helper(
                    &values[..split_idx],
                    &counts[..split_idx],
                    proof_left,
                    g_right,
                    bar,
                )
                .0
            },
            || {
                precompute_helper(
                    &values[split_idx..],
                    &counts[split_idx..],
                    proof_right,
                    g_left,
                    bar,
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
                bar,
            )
            .0,
            precompute_helper(
                &values[split_idx..],
                &counts[split_idx..],
                proof_right,
                g_left,
                bar,
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
    let (gcd, a, b) = Integer::extended_gcd(1u8.into(), e_star.clone(), Integer::new());
    debug_assert_eq!(gcd, 1u8);
    let base = G::default() * &b;
    let proof = NonMembershipWitness { exp: a, base };

    let height: usize = values.len().ilog2().try_into().unwrap();
    let bar = ProgressBar::new((values.len() * height).try_into().unwrap());

    let ret = precompute_helper(values, counts, proof, G::default(), &bar);
    bar.finish();

    ret
}

impl<G: Group + TryFrom<rug::Integer> + 'static> RsaAccumulator<G> {
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

        // TODO(maybe): parallelize GCD
        // gcd(a1, b) = 1 and gcd(a2, b) =1 => gcd(a1 * a2, b) = 1

        // Bezout coefficients:
        // gcd: exp * s + value * t = 1
        let (gcd, s, t) = Integer::extended_gcd_ref(&self.exponent, value.borrow()).into();
        if gcd != 1u8 {
            unreachable!("value should be coprime with the exponent of the accumulator");
        }
        debug_assert!(&s < value.inner()); // s should be small-ish

        debug_assert_eq!(self.digest.value, G::one().clone() * &self.exponent);

        let d = G::default() * &t;

        debug_assert_eq!(
            &((self.digest.value.clone() * &s) + (d.clone() * value.inner())),
            G::one(),
            "initially generating nonmembership proof failed"
        );

        Some(NonMembershipWitness { exp: s, base: d })
    }
}

impl<G: Group + TryFrom<Integer> + 'static> Accumulator for RsaAccumulator<G>
where
    NonMembershipWitness<G>: DataSized,
    SkipList<HistoryEntry<G>>: DataSized,
    RsaAccumulatorDigest<G>: DataSized,
    Witness<G>: DataSized,
{
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
        let x: Integer = member.clone().into();
        self.exponent *= x;
        self.multiset.insert(member.clone());

        self.history.add(HistoryEntry {
            end_digest: self.digest.clone(),
            exponent: member.into(),
        });

        // Update the digest history.
        self.digests_to_indexes
            .insert(self.digest.clone(), self.history.len() - 1);

        debug_assert_eq!(self.digest.value, G::one().clone() * &self.exponent);
        // Invalidate the nonmembership proof cache.
        self.nonmember_proof_cache = Default::default();
    }

    #[must_use]
    fn prove_append_only(&self, prefix: &Self::Digest) -> Self::AppendOnlyWitness {
        if &self.digest == prefix {
            panic!("identical");
        }
        let cur_idx = *self.digests_to_indexes.get(prefix).unwrap();
        let idx = self.history.len() - 1;

        let proof_value_list = self.history.read(cur_idx, idx);

        AppendOnlyWitness {
            inner: proof_value_list
                .into_iter()
                .map(|(a, b)| (a, b.end_digest.value))
                .collect(),
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
        let mut history = SkipList::<HistoryEntry<G>>::new();
        history.add(HistoryEntry {
            end_digest: digest.clone(),
            exponent: exponent.clone(),
        });
        let mut digests_to_indexes: HashMap<RsaAccumulatorDigest<G>, usize> = Default::default();
        digests_to_indexes.insert(digest.clone(), 0);
        debug_assert_eq!(digest.value, G::one().clone() * &exponent);
        Self {
            digest: digest.clone(),
            multiset,
            proof_cache,
            nonmember_proof_cache: Default::default(),
            history,
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
        let mut cur = new_state.value.clone();
        for (inner_proof, value) in proof.inner.iter().rev() {
            let zku = poke::ZKUniverse::<G>::default();
            let instance = poke::Instance {
                w: cur,
                u: value.clone(),
            };
            if !zku.verify(instance, inner_proof.clone()) {
                return false;
            }
            cur = value.clone();
        }
        cur == digest.value
    }

    fn cdn_size(&self) -> Information {
        let mut size = uom::ConstZero::ZERO;
        for (key, value) in &self.nonmember_proof_cache {
            size += key.size();
            size += value.size();
        }
        for (key, value) in &self.proof_cache {
            size += key.size();
            size += value.size();
        }

        size += self.history.size();

        size
    }
}

/*
#[cfg(test)]
use proptest::prelude::*;

#[cfg(test)]
impl<G> Arbitrary for RsaAccumulator<G>
where
    RsaAccumulator<G>: std::fmt::Debug + Clone + Default,
{
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
    /*
        #[test]
        fn test_rsa_accumulator_default() {
            let acc = RsaAccumulator::default();
            assert_eq!(acc.digest.value, GENERATOR.clone());
        }
    */
}
*/
