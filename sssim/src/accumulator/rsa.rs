#![allow(dead_code)]
use crate::accumulator::{Accumulator as AccumulatorTrait, BatchAccumulator};
use crate::poke;
use crate::primitives::{Collector, Group, Prime, SkipList};
use crate::util::assume_data_size_for_map;
use crate::util::{assume_data_size_for_vec, DataSized};
use crate::{multiset::MultiSet, util::Information};
use rayon::prelude::*;
use rug::Complete;
use rug::{ops::Pow, Integer};
use serde::Serialize;
use std::collections::HashMap;
use std::iter::zip;
use uom::ConstZero;

use indicatif::ProgressBar;

#[derive(Clone, Default, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct Digest<G>(G);

impl<G> DataSized for Digest<G>
where
    G: DataSized,
{
    fn size(&self) -> Information {
        self.0.size()
    }
}

#[derive(Clone, Serialize, Debug)]
struct MembershipWitness<G>(G);

impl<G: Group> MembershipWitness<G> {
    fn update(&mut self, value: &Prime) {
        self.0 *= value.as_ref()
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
    fn new(exp: Integer, base: G) -> Self {
        Self { exp, base }
    }

    /// Update this nonmembership witness for `value` with respect to `digest`.
    ///
    /// The result will be a valid nonmembership witness for `value` with
    /// respect to a new digest that has `new_element` added in.
    ///
    /// From [LLX07].
    fn update(&mut self, value: &Prime, new_element: Prime, digest: Digest<G>) {
        debug_assert!(digest.verify_nonmember(value.inner(), self.clone()));

        // If we're adding another copy of *this* value to the accumulator, no
        // update is necessary (the proof is still against the same digest as
        // before!).
        if value == &new_element {
            return;
        }

        // new_exp * member + _ * value = 1
        let (gcd, s, t) = Integer::extended_gcd_ref(value.inner(), new_element.as_ref()).into();
        debug_assert_eq!(gcd, 1u8);

        let (q, r) = (&self.exp * t).div_rem_ref(value.inner()).complete();
        let new_exp = r;

        let mut new_base = self.base.clone();
        new_base += digest.0.clone() * &(q * new_element.inner() + self.exp.clone() * s);

        self.exp = new_exp;
        self.base = new_base;

        debug_assert!({
            let new_digest = Digest(digest.0 * new_element.as_ref());
            new_digest.verify_nonmember(value.inner(), self.clone())
        });
    }

    fn for_one() -> Self {
        Self {
            exp: 1.into(),
            base: G::zero().clone(),
        }
    }

    fn prove(exponent: &Integer, nonmember: &Integer) -> Self {
        let (gcd, s, t) = Integer::extended_gcd_ref(exponent, nonmember).into();
        debug_assert_eq!(gcd, 1u8);
        debug_assert!(&s < nonmember); // s should be small-ish

        let d = G::default() * &t;

        let witness = NonMembershipWitness { exp: s, base: d };
        debug_assert!(
            Digest(G::default() * exponent).verify_nonmember(&nonmember.clone(), witness.clone()),
        );
        witness
    }

    /// Split this nonmembership proof for `e_l * e_r` with respect to `digest`.
    ///
    /// Result will be a nonmembership proof for `e_l` with respect to a new
    /// digest: `digest` with `members` added.
    ///
    /// If `members == e_r`, this is [TXN20, §3.2]: "Computing all
    /// Non-membership Witnesses Across Different, Related Accumulators."
    fn split(
        &self,
        digest: &Digest<G>,
        e_l: &Integer,
        e_r: &Integer,
        members: &Integer,
    ) -> (Digest<G>, Self) {
        debug_assert!(digest.verify_nonmember(&(e_l * e_r).into(), self.clone()));

        // s * e_l + t * members = 1
        let (gcd, s, t) = Integer::extended_gcd_ref(e_l, members).into();
        debug_assert_eq!(gcd, 1u8);

        let new_digest = Digest(digest.0.clone() * members);

        // => a = a * s * e_l + a * t * e_r
        let at = self.exp.clone() * t;
        let mut b = (digest.0.clone() * &self.exp * &s) + (self.base.clone() * e_r);
        debug_assert!({
            let new_proof_unreduced = NonMembershipWitness::new(at.clone(), b.clone());
            new_digest.verify_nonmember(e_l, new_proof_unreduced)
        });

        // reduce a * t mod e_l
        let (q, r) = at.div_rem(e_l.clone());
        b += digest.0.clone() * &(q * members.clone());
        let new_proof = NonMembershipWitness { exp: r, base: b };
        debug_assert!(&new_proof.exp < e_l);
        debug_assert!(new_digest.verify_nonmember(e_l, new_proof.clone()));

        (new_digest, new_proof)
    }
}

#[derive(Debug)]
struct Member {
    pub index: Integer,
    pub count: u32,
    /// index^count
    pub value: Integer,
}

impl Member {
    fn new(index: Integer, count: u32) -> Self {
        let value = index.clone().pow(count).into();
        Self {
            index,
            count,
            value,
        }
    }
}

/// Batch computation on accumulators often requires collecting the product of a
/// bunch of indexes and exponents.
#[derive(PartialEq, Eq, Debug)]
struct Intermediate {
    pub index: Integer,
    pub exponent: Integer,
}

impl Intermediate {
    fn from_members(members: &[Member]) -> Self {
        let mut index = Integer::from(1u8);
        let mut exponent = Integer::from(1u8);
        for member in members {
            index *= &member.index;
            exponent *= &member.value;
        }

        Self { index, exponent }
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

impl<G: Group + 'static> Digest<G> {
    fn for_members(members: &[Member]) -> Self {
        let mut g = G::default();
        for member in members {
            g *= &member.value;
        }
        Self(g)
    }

    fn for_exponent(exponent: &Integer) -> Self {
        Self(G::default() * exponent)
    }

    fn accumulate(&mut self, member: &Member) {
        self.0 *= &member.value;
    }

    fn accumulate_many(&mut self, members: &[Member]) {
        members.into_iter().for_each(|m| self.accumulate(m));
    }

    fn verify_member(&self, index: &Integer, count: u32, witness: MembershipWitness<G>) -> bool {
        let member = Integer::from(index.pow(count));
        witness.0 * &member == self.0
    }

    fn verify_member_option(&self, member: &Member, witness: Option<MembershipWitness<G>>) -> bool {
        match witness {
            Some(inner) => self.verify_member(&member.index, member.count, inner),
            None => member.count == 0,
        }
    }

    #[allow(non_snake_case)]
    #[must_use]
    fn verify_nonmember(&self, member: &Integer, witness: NonMembershipWitness<G>) -> bool {
        // https://link.springer.com/content/pdf/10.1007/978-3-540-72738-5_17.pdf
        let l = self.0.clone() * &witness.exp;
        let r = witness.base * &member;
        &(l + r) == G::one()
    }

    fn verify(&self, member: &Member, witness: Witness<G>) -> bool {
        match witness.member {
            Some(mem_pf) => {
                self.verify_member(&member.index, member.count, mem_pf.clone())
                    && Digest(mem_pf.0).verify_nonmember(&member.index, witness.nonmember)
            }
            None => {
                // Special-case: revision = 0 has no membership proof.
                member.count == 0 && self.verify_nonmember(&member.index, witness.nonmember)
            }
        }
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
    fn size(&self) -> Information {
        assume_data_size_for_vec(&self.inner)
    }
}

#[derive(Clone, Serialize, Debug)]
pub struct BatchWitness<W> {
    inner: HashMap<Prime, W>,
}

impl<W: DataSized> DataSized for BatchWitness<W> {
    fn size(&self) -> Information {
        assume_data_size_for_map(&self.inner)
    }
}

impl<G: Group + TryFrom<Integer> + 'static> BatchAccumulator for Accumulator<G>
where
    Accumulator<G>: AccumulatorTrait<Digest = Digest<G>>,
    BatchWitness<<Self as AccumulatorTrait>::Witness>: Clone,
{
    type BatchDigest = Digest<G>;
    type BatchWitness = BatchWitness<Self::Witness>;

    fn prove_batch<I: IntoIterator<Item = Prime>>(
        &mut self,
        entries: I,
    ) -> (HashMap<Prime, u32>, Self::BatchWitness) {
        // TODO(meh): do better using BBF19
        //
        // This only improves the *size* of the BatchWitness (and the
        // verification time); neither of these seems to be a bottleneck.
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

    /// Increment batch.
    ///
    /// Throughout, let n be the count of *existing* entries in the accumulator
    /// and let k be the number of unique elements in the batch.
    fn increment_batch<I: IntoIterator<Item = Prime>>(
        &mut self,
        members: I,
    ) -> Option<Self::AppendOnlyWitness> {
        let old_digest = self.digest.clone();

        // multiplicity of each member in the batch (size k)
        let mut members_hashmap = HashMap::<Prime, u32>::default();
        // exponent: product of all members in the batch
        let exponent = {
            let mut exponent = Integer::from(1u8);
            for member in members {
                exponent *= member.inner().clone();
                *members_hashmap.entry(member).or_insert(0) += 1;
            }
            Prime::new_unchecked(exponent)
        };

        // TODO(meh):
        // idea: k log k trick to multiply everything together
        // [0]   [1]   [2]   [3] ->
        // [123] [023] [013] [012]

        // Update all *existing* proofs for this batch (O(n), fixed-ish amount
        // of work in each iteration).
        self.proof_cache.par_iter_mut().for_each(|(value, proof)| {
            let digest = Digest(proof.member.clone().unwrap().0);
            proof.nonmember.update(value, exponent.clone(), digest);

            // for all members that equal value: divide by member
            let update_val = match members_hashmap.get(value) {
                Some(count) => Prime::new_unchecked(
                    exponent.clone().into_inner() / Integer::from(value.inner().pow(count)),
                ),
                None => exponent.clone(),
            };
            proof.member.as_mut().unwrap().update(&update_val);
        });

        // This is O(k)-ish. Each iteration takes a fixed-ish amount of time for
        // RSA group multiplications. Can't think of any way to batch this.
        let newly_added = members_hashmap
            .iter()
            .filter_map(|(member, count)| {
                if self.proof_cache.get_mut(member).is_none() {
                    let nonmember_proof = self
                        .nonmember_proof_cache
                        .remove(&member)
                        .expect("we compute nonmembership proofs as-we-go");
                    return Some((member.clone(), *count, nonmember_proof));
                }
                None
            })
            .collect::<Vec<_>>();
        let new_proofs: Vec<(Prime, Witness<_>)> = newly_added
            .into_par_iter()
            .map(|(member, count, mut nonmember_proof)| {
                let exponent = exponent.inner().clone() / member.inner().pow(count).complete();
                let membership_proof = MembershipWitness(self.digest.0.clone() * &exponent);
                nonmember_proof.update(&member, Prime::new_unchecked(exponent), old_digest.clone());
                let proof = Witness {
                    member: Some(membership_proof),
                    nonmember: nonmember_proof,
                };
                (member, proof)
            })
            .collect();
        for (member, proof) in new_proofs {
            self.proof_cache.insert(member, proof);
        }

        self.digest.0 *= exponent.inner();
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
        debug_assert_eq!(self.digest.0, G::one().clone() * &self.exponent);

        Some(self.prove_append_only(&old_digest))
    }

    fn verify_batch(
        digest: &Self::BatchDigest,
        members: &HashMap<Prime, u32>,
        mut witness: Self::BatchWitness,
    ) -> bool {
        // TODO(probably not): do better using BBF19?
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
        true
    }
}

#[derive(Clone, Debug)]
pub struct HistoryEntry<G> {
    exponent: Integer,
    end_digest: Digest<G>,
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
            w: self.end_digest.0.clone(),
            u: item.end_digest.0.clone(),
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
    Digest<G>: DataSized,
{
    fn size(&self) -> Information {
        self.exponent.size() + self.end_digest.size()
    }
}

// TODO(maybe): shard storage across # cores
#[derive(Default, Debug, Clone)]
pub struct Accumulator<G>
where
    HistoryEntry<G>: Collector,
    SkipList<HistoryEntry<G>>: std::fmt::Debug,
{
    digest: Digest<G>,
    multiset: MultiSet<Prime>,
    proof_cache: HashMap<Prime, Witness<G>>,
    nonmember_proof_cache: HashMap<Prime, NonMembershipWitness<G>>,
    history: SkipList<HistoryEntry<G>>,
    digests_to_indexes: HashMap<Digest<G>, usize>,
    exponent: Integer,
}

impl<G> DataSized for Accumulator<G>
where
    HistoryEntry<G>: Collector,
    SkipList<HistoryEntry<G>>: std::fmt::Debug,
    SkipList<HistoryEntry<G>>: DataSized,
    Digest<G>: DataSized,
    Witness<G>: DataSized,
    NonMembershipWitness<G>: DataSized,
{
    fn size(&self) -> Information {
        let mut size = self.digest.size() + self.history.size() + self.exponent.size();
        size += self.multiset.size();
        size += assume_data_size_for_map(&self.proof_cache);
        size += assume_data_size_for_map(&self.nonmember_proof_cache);
        size += assume_data_size_for_map(&self.digests_to_indexes);
        size
    }
}

fn precompute_helper<G: Group + 'static>(
    members: &[Member],
    foo: &Intermediate,
    proof: NonMembershipWitness<G>,
    digest: Digest<G>,
    bar: &ProgressBar,
) -> Vec<Witness<G>> {
    debug_assert!(!members.is_empty());
    debug_assert_eq!(foo, &Intermediate::from_members(members));
    debug_assert!(digest.verify_nonmember(&foo.index, proof.clone()));

    if members.len() == 1 {
        return vec![Witness::new(MembershipWitness(digest.0), proof)];
    }

    let (l, r) = members.split_at(members.len() / 2);
    let foo_l = Intermediate::from_members(&l);
    let foo_r = Intermediate::from_members(&r);

    let (digest_r, proof_l) = proof.split(&digest, &foo_l.index, &foo_r.index, &foo_r.exponent);
    debug_assert!(digest_r.verify_nonmember(&foo_l.index.clone(), proof_l.clone()));
    debug_assert!(digest_r.verify_member(&foo_r.exponent, 1, MembershipWitness(digest.0.clone())));
    let (digest_l, proof_r) = proof.split(&digest, &foo_r.index, &foo_l.index, &foo_l.exponent);
    debug_assert!(digest_l.verify_nonmember(&foo_r.index.clone(), proof_r.clone()));
    debug_assert!(digest_l.verify_member(&foo_l.exponent, 1, MembershipWitness(digest.0.clone())));

    bar.inc(members.len().try_into().unwrap());

    let (mut ret, r_ret) = rayon::join(
        || precompute_helper(&l, &foo_l, proof_l, digest_r, bar),
        || precompute_helper(&r, &foo_r, proof_r, digest_l, bar),
    );
    ret.extend_from_slice(&r_ret);

    ret
}

/// Returns (Vec<Witness>, digest, exponent)
fn precompute<G: Group + 'static>(
    members: &[Member],
) -> (Vec<Witness<G>>, Digest<G>, Intermediate) {
    let foo = Intermediate::from_members(members);
    if members.len() == 0 {
        return (vec![], Default::default(), foo);
    }

    let exponent = Integer::from(1u8);
    let digest = Digest::for_exponent(&exponent);
    let proof = NonMembershipWitness::prove(&exponent, &foo.exponent); // TODO: for_one()
    debug_assert!(digest.verify_nonmember(&foo.exponent, proof.clone()));

    let bar = if false {
        let height: usize = members.len().ilog2().try_into().unwrap();
        ProgressBar::new((members.len() * height).try_into().unwrap())
    } else {
        ProgressBar::hidden()
    };

    let witnesses = precompute_helper(members, &foo, proof, digest, &bar);
    bar.finish();

    let digest = Digest::for_members(members);
    debug_assert!(
        zip(members, &witnesses).all(|(member, witness)| digest.verify(member, witness.clone()))
    );

    (witnesses, digest, foo)
}

impl<G: Group + TryFrom<rug::Integer> + 'static> Accumulator<G> {
    #[must_use]
    fn prove_member(&self, member: &Prime, revision: u32) -> Option<MembershipWitness<G>> {
        debug_assert!(<Prime as AsRef<Integer>>::as_ref(member) >= &0);
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

        // TODO(probably not): parallelize GCD
        // gcd(a1, b) = 1 and gcd(a2, b) =1 => gcd(a1 * a2, b) = 1

        // Bezout coefficients:
        // gcd: exp * s + value * t = 1
        let (gcd, s, t) = Integer::extended_gcd_ref(&self.exponent, value.as_ref()).into();
        if gcd != 1u8 {
            unreachable!("value should be coprime with the exponent of the accumulator");
        }
        debug_assert!(&s < value.inner()); // s should be small-ish

        debug_assert_eq!(self.digest.0, G::one().clone() * &self.exponent);

        let d = G::default() * &t;

        debug_assert_eq!(
            &((self.digest.0.clone() * &s) + (d.clone() * value.inner())),
            G::one(),
            "initially generating nonmembership proof failed"
        );

        Some(NonMembershipWitness { exp: s, base: d })
    }
}

impl<G: Group + TryFrom<Integer> + 'static> AccumulatorTrait for Accumulator<G>
where
    NonMembershipWitness<G>: DataSized,
    SkipList<HistoryEntry<G>>: DataSized,
    Digest<G>: DataSized,
    Witness<G>: DataSized,
{
    type Digest = Digest<G>;
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
            let digest = Digest(proof.member.clone().unwrap().0);
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
            let membership_proof = MembershipWitness(self.digest.0.clone());
            let proof = Witness {
                member: Some(membership_proof),
                nonmember: self.prove_nonmember(&member).unwrap(),
            };
            self.proof_cache.insert(member.clone(), proof);
        }

        // Update the digest to add the member.
        self.digest.0 *= member.as_ref();
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

        debug_assert_eq!(self.digest.0, G::one().clone() * &self.exponent);
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
                .map(|(a, b)| (a, b.end_digest.0))
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
        let members: Vec<_> = multiset
            .iter()
            .map(|(value, count)| Member::new(value.clone().into(), *count))
            .collect();
        let (proofs, digest, foo) = precompute(&members);

        let mut proof_cache: HashMap<Prime, Witness<G>> = Default::default();
        for (member, witness) in zip(members, proofs) {
            proof_cache.insert(Prime::new_unchecked(member.index.clone()), witness);
        }

        let mut history = SkipList::<HistoryEntry<G>>::new();
        history.add(HistoryEntry {
            end_digest: digest.clone(),
            exponent: foo.exponent.clone(),
        });
        let mut digests_to_indexes: HashMap<Digest<G>, usize> = Default::default();
        digests_to_indexes.insert(digest.clone(), 0);
        debug_assert_eq!(digest.0, G::default() * &foo.exponent);
        Self {
            digest,
            multiset,
            proof_cache,
            nonmember_proof_cache: Default::default(),
            history,
            digests_to_indexes,
            exponent: foo.exponent,
        }
    }

    #[must_use]
    fn verify(digest: &Self::Digest, index: &Prime, revision: u32, witness: Self::Witness) -> bool {
        // member@revision is valid IF
        // (a) member@revision is in the set and
        // (b) member is NOT in the set corresponding to the membership proof for (a)

        match witness.member {
            Some(mem_pf) => {
                digest.verify_member(&index.inner(), revision, mem_pf.clone())
                    && Digest(mem_pf.0).verify_nonmember(index.as_ref(), witness.nonmember)
            }
            None => {
                // Special-case: revision = 0 has no membership proof.
                revision == 0 && digest.verify_nonmember(index.as_ref(), witness.nonmember)
            }
        }
    }

    #[must_use]
    fn verify_append_only(
        digest: &Self::Digest,
        proof: &Self::AppendOnlyWitness,
        new_state: &Self::Digest,
    ) -> bool {
        let mut cur = new_state.0.clone();
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
        cur == digest.0
    }

    fn cdn_size(&self) -> Information {
        let mut size = Information::ZERO;
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

///
///
/// [0]   [1]   [2]   [3] ->
/// [123] [023] [013] [012]
pub fn multiply_stuff(values: &[Integer], counts: &[u32]) -> (Vec<Integer>, Integer) {
    debug_assert_eq!(values.len(), counts.len());
    debug_assert!(!values.is_empty());

    let mut result = vec![Integer::from(1); values.len()];
    let mut total = Integer::from(1);
    for i in 0..values.len() {
        let value = values[i].clone().pow(counts[i]);
        for j in 0..values.len() {
            if i == j {
                continue;
            }
            result[j] *= value.clone()
        }
        total *= value;
    }

    (result, total)
}

// ///
// ///
// /// [0]   [1]   [2]   [3] ->
// /// [123] [023] [013] [012]
// pub fn multiply_stuff_recursive(values: &[Integer], counts: &[u32]) -> (Vec<Integer>, Integer) {
//     // TODO: fix so this is actually faster
//     if values.len() == 1 {
//         let exponent = Integer::from(values[0].clone()) * counts[0];
//         return vec![Integer::from(members_star)];
//     }
//
//     let split_idx = values.len() / 2;
//     let mut members_left = Integer::from(1u8);
//     let mut members_right = Integer::from(1u8);
//     for idx in 0..split_idx {
//         let value = &values[idx];
//         let count = counts[idx];
//         members_left *= Integer::from(value.clone()).pow(count);
//     }
//     for idx in split_idx..values.len() {
//         let value = &values[idx];
//         let count = counts[idx];
//         members_right *= Integer::from(value.clone()).pow(count);
//     }
//     // let members_star = members_left * members_right; // TODO: maybe use this?
//
//     let (mut ret, r_ret) = (
//         multiply_stuff_recursive(&values[..split_idx], &counts[..split_idx]),
//         multiply_stuff_recursive(&values[split_idx..], &counts[split_idx..]),
//     );
//     ret.extend_from_slice(&r_ret);
//     ret
// }

pub fn multiply_stuff2(values: &[Integer], counts: &[u32]) -> (Vec<Integer>, Integer) {
    let mut total = Integer::from(1);
    for i in 0..values.len() {
        let value = values[i].clone().pow(counts[i]);
        total *= value;
    }
    let mut results = vec![total.clone(); values.len()];
    for i in 0..values.len() {
        let value = values[i].clone().pow(counts[i]);
        results[i] /= value;
    }
    (results, total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_multiply_stuff() {
        let values: Vec<Integer> = vec![2, 3].into_iter().map(Integer::from).collect();
        let counts: Vec<u32> = vec![1, 1];
        assert_eq!(
            multiply_stuff(&values, &counts),
            (vec![Integer::from(3), Integer::from(2)], Integer::from(6))
        );
    }

    fn values_with_counts() -> impl Strategy<Value = (Vec<Integer>, Vec<u32>)> {
        use prop::collection::vec;
        (1..10usize).prop_flat_map(|length| {
            (
                vec((1..10u8).prop_map(Integer::from), length),
                vec(1..10u32, length),
            )
        })
    }

    proptest! {
        #[test]
        fn test_multiplies_the_same((values, counts) in values_with_counts()) {
            assert_eq!(
                multiply_stuff(&values, &counts),
                multiply_stuff2(&values, &counts),
            );
        }
    }

    type G = crate::primitives::RsaGroup;

    fn multisets() -> impl Strategy<Value = MultiSet<Prime>> {
        (0usize..=10)
            .prop_flat_map(|len| vec![any::<Prime>(); len])
            .prop_map(MultiSet::from)
    }

    proptest! {
        #[test]
        fn test_accumulator_members(multiset in multisets()) {
            let mut acc = Accumulator::<G>::import(multiset.clone());

            let digest = acc.digest.clone();
            for (index, count) in multiset.iter() {
                let proof = acc.prove(index, *count).unwrap();
                let member = Member::new(index.clone().into(), *count);
                prop_assert!(digest.verify(&member, proof));
            }
        }
    }
}
