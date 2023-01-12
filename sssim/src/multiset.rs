//!Multiset

use std::{collections::HashMap, hash::Hash};

use rug::Integer;
use serde::{ser::SerializeMap, Serialize};

#[derive(Debug, Clone)]
pub struct MultiSet<T: Hash + Eq> {
    pub inner: HashMap<T, u32>,
}

impl<T: Hash + Eq> Default for MultiSet<T> {
    fn default() -> Self {
        Self {
            inner: Default::default(),
        }
    }
}

impl Serialize for MultiSet<Integer> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.inner.len()))?;
        for (k, v) in &self.inner {
            map.serialize_entry(&k.to_string(), v)?;
        }
        map.end()
    }
}

impl<T: Hash + Eq> MultiSet<T> {
    pub fn insert(&mut self, member: T) {
        *self.inner.entry(member).or_insert(0) += 1;
    }

    pub fn get(&self, member: &T) -> u32 {
        *self.inner.get(member).unwrap_or(&0)
    }

    pub fn remove(&mut self, member: &T) -> bool {
        let value = self.inner.get_mut(member);
        match value {
            Some(v) if *v > 1 => {
                *v -= 1;
                true
            }
            Some(v) if *v == 1 => {
                self.inner.remove(member);
                true
            }
            Some(_) => panic!("invalid; should never have 0 value in hashmap"),
            None => false,
        }
    }

    pub fn clear(&mut self, member: &T) -> Option<u32> {
        self.inner.remove(member)
    }

    pub fn iter<'a>(&'a self) -> impl std::iter::Iterator<Item = (&'a T, &u32)> {
        self.inner.iter()
    }

    pub fn is_superset(&self, other: &Self) -> bool {
        for (key, count) in other.inner.iter() {
            if self.inner.get(key).unwrap_or(&0) < count {
                return false;
            }
        }
        true
    }

    pub fn difference<'a>(&'a self, other: &Self) -> Vec<(&'a T, u32)> {
        // TODO(maybe): make it return a real iterator
        let mut results: Vec<(&T, u32)> = vec![];
        for (key, count) in self.inner.iter() {
            let diff: u32 = count
                .checked_sub(*other.inner.get(key).unwrap_or(&0))
                .expect("not a superset");
            results.push((key, diff))
        }
        results
    }

    pub fn len(&self) -> usize {
        return self.inner.len();
    }
}

impl<T: Hash + Eq + Clone + Default> From<Vec<T>> for MultiSet<T> {
    fn from(values: Vec<T>) -> Self {
        let mut multiset = MultiSet::default();
        for value in values.iter().cloned() {
            multiset.insert(value);
        }
        multiset
    }
}
