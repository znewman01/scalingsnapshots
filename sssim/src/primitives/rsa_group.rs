use super::{AdaptiveRootAssumption, Group};
use crate::util::{DataSized, Information};
use once_cell::sync::Lazy;
use rug::Integer;
use serde::Serialize;
use std::ops::{Add, AddAssign, Deref, Mul, MulAssign};

static MODULUS: Lazy<Integer> = Lazy::new(|| {
    Integer::parse(
        "2519590847565789349402718324004839857142928212620403202777713783604366202070\
           7595556264018525880784406918290641249515082189298559149176184502808489120072\
           8449926873928072877767359714183472702618963750149718246911650776133798590957\
           0009733045974880842840179742910064245869181719511874612151517265463228221686\
           9987549182422433637259085141865462043576798423387184774447920739934236584823\
           8242811981638150106748104516603773060562016196762561338441436038339044149526\
           3443219011465754445417842402092461651572335077870774981712577246796292638635\
           6373289912154831438167899885040445364023527381951378636564391212010397122822\
           120720357",
    )
    .unwrap()
    .into()
});

/// The multiplicative group of integers mod RSA-2048.
///
/// A couple of false positives (not co-prime with the modulus), but hitting
/// them implies that we've factored RSA-2048.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize)]
pub struct Rsa2048Group {
    value: Integer,
}

impl Rsa2048Group {
    /// Check that this is a valid group element.
    fn check_value(&self) -> bool {
        0u8 < self.value && &self.value <= MODULUS.deref()
    }
}

impl DataSized for Rsa2048Group {
    fn size(&self) -> Information {
        self.value.size()
    }
}

impl Default for Rsa2048Group {
    fn default() -> Self {
        Self {
            value: Integer::from(65337),
        }
    }
}

impl TryFrom<Integer> for Rsa2048Group {
    type Error = ();

    fn try_from(value: Integer) -> Result<Self, Self::Error> {
        let x = Self { value };
        match x.check_value() {
            true => Ok(x),
            false => Err(()),
        }
    }
}

#[cfg(test)]
use proptest::prelude::*;
#[cfg(test)]
impl Arbitrary for Rsa2048Group {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        // any::<u16>()
        Just(0u16)
            .prop_map(|exp| exp.saturating_add(1))
            .prop_map(|exp| Rsa2048Group::default() * &(exp.into()))
            .boxed()
    }
}

impl Add<Self> for Rsa2048Group {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self::Output {
        self += rhs;
        self
    }
}

impl AddAssign<Self> for Rsa2048Group {
    fn add_assign(&mut self, rhs: Self) {
        self.value *= rhs.value;
        self.value %= MODULUS.deref();
        assert!(self.check_value());
    }
}

impl Mul<&Integer> for Rsa2048Group {
    type Output = Self;

    fn mul(mut self, rhs: &Integer) -> Self::Output {
        self *= &rhs;
        self
    }
}

impl MulAssign<&Integer> for Rsa2048Group {
    fn mul_assign(&mut self, rhs: &Integer) {
        self.value
            .pow_mod_mut(rhs, MODULUS.deref())
            .expect("exp > 0, MODULUS > 0");
        assert!(self.check_value());
    }
}

static ZERO: Lazy<Rsa2048Group> = Lazy::new(|| Integer::from(1).try_into().unwrap());
static GENERATOR: Lazy<Rsa2048Group> = Lazy::new(|| Integer::from(65337).try_into().unwrap());
static MAX_VALUE: Lazy<Rsa2048Group> =
    Lazy::new(|| Integer::from(MODULUS.deref() - 1).try_into().unwrap());

impl Group for Rsa2048Group {
    fn zero() -> &'static Self {
        &ZERO
    }

    fn one() -> &'static Self {
        &GENERATOR
    }

    fn max_value() -> &'static Self {
        &MAX_VALUE
    }

    fn bytes() -> usize {
        MAX_VALUE.value.significant_digits::<u8>()
    }
}

impl AdaptiveRootAssumption for Rsa2048Group {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::group;

    group::check_laws!(Rsa2048Group);
}
