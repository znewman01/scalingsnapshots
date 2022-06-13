use std::collections::HashMap;

use rug::Integer;
use serde::{ser::SerializeMap, Serialize};

use crate::accumulator::{Accumulator, Digest};

#[derive(Default, Debug, Clone)]
struct Cache<A>
where
    A: Accumulator + Serialize,
    <A as Accumulator>::Digest:
        Eq + PartialEq + std::hash::Hash + std::fmt::Debug + Clone + Serialize,
    <<A as Accumulator>::Digest as Digest>::Witness: std::fmt::Debug + Clone + Serialize,
{
    inner: HashMap<
        (<A as Accumulator>::Digest, Integer, u32),
        Option<<<A as Accumulator>::Digest as Digest>::Witness>,
    >,
}

impl<A> Serialize for Cache<A>
where
    A: Accumulator + Serialize,
    <A as Accumulator>::Digest:
        Eq + PartialEq + std::hash::Hash + std::fmt::Debug + Clone + Serialize,
    <<A as Accumulator>::Digest as Digest>::Witness: std::fmt::Debug + Clone + Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.inner.len()))?;
        for (k, v) in &self.inner {
            map.serialize_entry(&format!("{:?}:{}:{}", k.0, k.1, k.2), v)?;
        }
        map.end()
    }
}

#[derive(Default, Debug, Clone, Serialize)]
pub struct CachingAccumulator<A>
where
    A: Accumulator + Serialize,
    <A as Accumulator>::Digest:
        Eq + PartialEq + std::hash::Hash + std::fmt::Debug + Clone + Serialize,
    <<A as Accumulator>::Digest as Digest>::Witness: std::fmt::Debug + Clone + Serialize,
{
    acc: A,
    cache: Cache<A>,
}

impl<A> Accumulator for CachingAccumulator<A>
where
    A: Accumulator + Serialize,
    <A as Accumulator>::Digest:
        Eq + PartialEq + std::hash::Hash + std::fmt::Debug + Clone + Serialize,
    <<A as Accumulator>::Digest as Digest>::Witness: Clone + std::fmt::Debug + Serialize,
{
    type Digest = A::Digest;

    #[must_use]
    fn digest(&self) -> &Self::Digest {
        self.acc.digest()
    }

    fn increment(&mut self, member: Integer) {
        self.acc.increment(member);
    }

    #[must_use]
    fn prove_append_only_from_vec(
        &self,
        other: &[Integer],
    ) -> <<CachingAccumulator<A> as Accumulator>::Digest as Digest>::AppendOnlyWitness {
        self.acc.prove_append_only_from_vec(other)
    }

    #[must_use]
    fn prove_append_only(&self, other: &Self) -> Integer {
        self.acc.prove_append_only(&other.acc)
    }

    fn prove(
        &mut self,
        member: &Integer,
        revision: u32,
    ) -> Option<<<Self as Accumulator>::Digest as Digest>::Witness> {
        match self
            .cache
            .inner
            .get(&(self.digest().clone(), member.clone(), revision))
        {
            Some(w) => w.clone(),
            None => {
                let witness = self.acc.prove(member, revision);
                self.cache.inner.insert(
                    (self.digest().clone(), member.clone(), revision),
                    witness.clone(),
                );
                witness
            }
        }
    }

    fn get(&self, member: &Integer) -> u32 {
        self.acc.get(member)
    }
}
