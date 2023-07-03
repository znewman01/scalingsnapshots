use rug::Integer;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("value is zero")]
    Zero,
    #[error("value is negative")]
    Negative(Integer),
}

pub trait NonZero: AsRef<Integer> + Into<Integer> {}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NonZeroInteger(Integer);

impl NonZero for NonZeroInteger {}

impl TryFrom<Integer> for NonZeroInteger {
    type Error = Error;

    fn try_from(value: Integer) -> Result<Self, Self::Error> {
        if value == 0 {
            return Err(Error::Zero);
        }
        Ok(NonZeroInteger(value))
    }
}

impl From<NonZeroInteger> for Integer {
    fn from(value: NonZeroInteger) -> Integer {
        value.0
    }
}

impl AsRef<Integer> for NonZeroInteger {
    fn as_ref(&self) -> &Integer {
        &self.0
    }
}

pub trait NonNegative: AsRef<Integer> + Into<Integer> {}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NonNegativeInteger(Integer);

impl NonNegative for NonNegativeInteger {}

impl TryFrom<Integer> for NonNegativeInteger {
    type Error = Error;

    fn try_from(value: Integer) -> Result<Self, Self::Error> {
        if value < 0 {
            return Err(Error::Negative(value));
        }
        Ok(NonNegativeInteger(value))
    }
}

impl From<NonNegativeInteger> for Integer {
    fn from(value: NonNegativeInteger) -> Integer {
        value.0
    }
}

impl AsRef<Integer> for NonNegativeInteger {
    fn as_ref(&self) -> &Integer {
        &self.0
    }
}

pub trait Positive: NonNegative + NonZero {}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PositiveInteger(Integer);

impl NonNegative for PositiveInteger {}
impl NonZero for PositiveInteger {}

impl TryFrom<Integer> for PositiveInteger {
    type Error = Error;

    fn try_from(value: Integer) -> Result<Self, Self::Error> {
        use std::cmp::Ordering::*;
        match value.cmp(&Integer::ZERO) {
            Less => Err(Error::Negative(value)),
            Equal => Err(Error::Zero),
            Greater => Ok(PositiveInteger(value)),
        }
    }
}

impl From<PositiveInteger> for Integer {
    fn from(value: PositiveInteger) -> Integer {
        value.0
    }
}

impl AsRef<Integer> for PositiveInteger {
    fn as_ref(&self) -> &Integer {
        &self.0
    }
}
