//!Multiset

use std::{collections::HashMap, hash::Hash};

use serde::Serialize;

#[derive(Debug, Default, Clone, Serialize)]
pub struct MultiSet<T: Hash + Eq> {
    inner: HashMap<T, u32>,
}

impl<T: Hash + Eq> MultiSet<T> {
    pub fn insert(&mut self, member: T) {
        *self.inner.entry(member).or_insert(0) += 1;
    }

    pub fn get(&self, member: &T) -> u32 {
        *self.inner.get(member).unwrap_or(&0)
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
        return true;
    }

    pub fn difference<'a>(&'a self, other: &Self) -> Vec<(&'a T, u32)> {
        // TODO: make it return a real iterator
        let mut results: Vec<(&T, u32)> = vec![];
        for (key, count) in self.inner.iter() {
            let diff: u32 = count
                .checked_sub(*other.inner.get(key).unwrap_or(&0))
                .expect("not a superset");
            results.push((key, diff))
        }
        results
    }
}
