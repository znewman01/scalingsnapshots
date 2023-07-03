use rug::Integer;
use serde::Serialize;
use thiserror::Error;

use super::{NonNegative, NonZero};

use crate::util::DataSized;
use crate::util::Information;

// How sure do we want to be that our primes are actually prime?
// We want to be 30 sure.
const MILLER_RABIN_ITERS: u32 = 30;

#[derive(Error, Debug)]
#[error("{value} is composite")]
pub struct CompositeError {
    value: Integer,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize)]
pub struct Prime(Integer);

impl NonNegative for Prime {}
impl NonZero for Prime {}

impl Prime {
    /// e.g. for a prime power
    pub fn new_unchecked(value: Integer) -> Self {
        Self(value)
    }

    pub fn inner(&self) -> &Integer {
        &self.0
    }

    pub fn into_inner(self) -> Integer {
        self.0
    }
}

impl TryFrom<Integer> for Prime {
    type Error = CompositeError;

    fn try_from(value: Integer) -> Result<Self, Self::Error> {
        if value.is_probably_prime(MILLER_RABIN_ITERS) == rug::integer::IsPrime::No {
            return Err(CompositeError { value });
        }
        Ok(Prime(value))
    }
}

impl DataSized for Prime {
    fn size(&self) -> Information {
        self.0.size()
    }
}

impl From<Prime> for Integer {
    fn from(prime: Prime) -> Self {
        prime.0
    }
}

impl AsRef<Integer> for Prime {
    fn as_ref(&self) -> &Integer {
        &self.0
    }
}

#[cfg(test)]
use proptest::prelude::*;

#[cfg(test)]
impl Arbitrary for Prime {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop::sample::select(vec![2u32, 3, 5, 7, 11])
            .prop_map(Integer::from)
            .prop_map(Prime::try_from)
            .prop_map(Result::unwrap)
            .boxed()
    }
}
