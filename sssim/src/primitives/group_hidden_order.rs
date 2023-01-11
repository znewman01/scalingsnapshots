use super::Group;

/// Marker trait for Groups satisfying the adaptive root assumption [Wes18].
///
/// Also known as a hidden-order group.
///
/// [Wes18]: https://eprint.iacr.org/2018/623
pub trait AdaptiveRootAssumption: Group {}
