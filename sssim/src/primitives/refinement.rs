use std::borrow::Borrow;

use rug::Integer;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("value is zero")]
    Zero,
    #[error("value is negative")]
    Negative(Integer),
}

pub trait NonZero: Borrow<Integer> + Into<Integer> {}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NonZeroInteger {
    value: Integer,
}

impl NonZero for NonZeroInteger {}

impl TryFrom<Integer> for NonZeroInteger {
    type Error = Error;

    fn try_from(value: Integer) -> Result<Self, Self::Error> {
        if value == 0 {
            return Err(Error::Zero);
        }
        Ok(NonZeroInteger { value })
    }
}

impl From<NonZeroInteger> for Integer {
    fn from(value: NonZeroInteger) -> Integer {
        value.value
    }
}

impl Borrow<Integer> for NonZeroInteger {
    fn borrow(&self) -> &Integer {
        &self.value
    }
}

pub trait NonNegative: Borrow<Integer> + Into<Integer> {}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NonNegativeInteger {
    value: Integer,
}

impl NonNegative for NonNegativeInteger {}

impl TryFrom<Integer> for NonNegativeInteger {
    type Error = Error;

    fn try_from(value: Integer) -> Result<Self, Self::Error> {
        if value < 0 {
            return Err(Error::Negative(value));
        }
        Ok(NonNegativeInteger { value })
    }
}

impl From<NonNegativeInteger> for Integer {
    fn from(value: NonNegativeInteger) -> Integer {
        value.value
    }
}

impl Borrow<Integer> for NonNegativeInteger {
    fn borrow(&self) -> &Integer {
        &self.value
    }
}

pub trait Positive: NonNegative + NonZero {}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PositiveInteger {
    value: Integer,
}

impl NonNegative for PositiveInteger {}
impl NonZero for PositiveInteger {}

impl TryFrom<Integer> for PositiveInteger {
    type Error = Error;

    fn try_from(value: Integer) -> Result<Self, Self::Error> {
        use std::cmp::Ordering::*;
        match value.cmp(&Integer::ZERO) {
            Less => Err(Error::Negative(value)),
            Equal => Ok(PositiveInteger { value }),
            Greater => Err(Error::Zero),
        }
    }
}

impl From<PositiveInteger> for Integer {
    fn from(value: PositiveInteger) -> Integer {
        value.value
    }
}

impl Borrow<Integer> for PositiveInteger {
    fn borrow(&self) -> &Integer {
        &self.value
    }
}
