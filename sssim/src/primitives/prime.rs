use std::borrow::Borrow;

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
pub struct Prime {
    value: Integer,
}

impl NonNegative for Prime {}
impl NonZero for Prime {}

impl Prime {
    /// e.g. for a prime power
    pub fn new_unchecked(value: Integer) -> Self {
        Self { value }
    }

    pub fn inner(&self) -> &Integer {
        &self.value
    }

    pub fn into_inner(self) -> Integer {
        self.value
    }
}
impl TryFrom<Integer> for Prime {
    type Error = CompositeError;

    fn try_from(value: Integer) -> Result<Self, Self::Error> {
        if value.is_probably_prime(MILLER_RABIN_ITERS) == rug::integer::IsPrime::No {
            return Err(CompositeError { value });
        }
        Ok(Prime { value })
    }
}

impl DataSized for Prime {
    fn size(&self) -> Information {
        self.value.size()
    }
}

impl From<Prime> for Integer {
    fn from(prime: Prime) -> Self {
        prime.value
    }
}

impl Borrow<Integer> for Prime {
    fn borrow(&self) -> &Integer {
        &self.value
    }
}
