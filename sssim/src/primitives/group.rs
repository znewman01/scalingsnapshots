use rug::Integer;
use serde::Serialize;
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::{Add, AddAssign, Mul, MulAssign};

pub trait Group:
    Clone
    + Debug
    + Default
    + Eq
    + Hash
    + AddAssign<Self>
    + Add<Self, Output = Self>
    + for<'a> MulAssign<&'a Integer>
    + for<'a> Mul<&'a Integer, Output = Self>
    + Sync
    + Send
    + Serialize
{
    fn zero() -> &'static Self;
    fn one() -> &'static Self;
    fn max_value() -> &'static Self;
    fn bytes() -> usize;
}

#[cfg(test)]
macro_rules! check_laws {
    ($type:ty) => {
        mod group_laws {
            #![allow(unused_imports)]
            use super::*;
            use crate::primitives::Group;

            fn check_commutative<G: Group>(a: G, b: G) -> Result<(), TestCaseError> {
                let lhs = {
                    let (a, b) = (a.clone(), b.clone());
                    a + b
                };
                let rhs = b + a;
                prop_assert_eq!(&lhs, &rhs);
                Ok(())
            }

            fn check_associative<G: Group>(a: G, b: G, c: G) -> Result<(), TestCaseError> {
                let lhs = {
                    let (a, b, c) = (a.clone(), b.clone(), c.clone());
                    (a + b) + c
                };
                let rhs = a + (b + c);
                prop_assert_eq!(&lhs, &rhs);
                Ok(())
            }

            fn check_identity<G: Group + 'static>(a: G) -> Result<(), TestCaseError> {
                let zero = G::zero().clone();
                let lhs = {
                    let (a, zero) = (a.clone(), zero.clone());
                    a + zero
                };
                let rhs = zero + a.clone();
                prop_assert_eq!(&lhs, &a);
                prop_assert_eq!(&a, &rhs);
                Ok(())
            }

            fn check_add_assign<G: Group>(a: G, b: G) -> Result<(), TestCaseError> {
                let lhs = a.clone() + b.clone();
                let mut rhs = a;
                rhs += b;
                prop_assert_eq!(&lhs, &rhs);
                Ok(())
            }

            proptest! {
                #[test]
                fn test_commutative(a: $type, b: $type) {
                    check_commutative(a, b)?;
                }

                #[test]
                fn test_associative(a: $type, b: $type, c: $type) {
                    check_associative(a, b, c)?;
                }


                #[test]
                fn test_identity(a: $type) {
                    check_identity(a)?;
                }

                #[test]
                fn test_add_assign(a: $type, b: $type) {
                    check_add_assign(a, b)?;
                }
            }
        }
    };
}

#[cfg(test)]
pub(super) use check_laws;
