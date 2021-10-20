use crate::authenticator;

/// An insecure authenticator.
///
/// Useful for testing.
#[derive(Default)]
pub struct Authenticator {}

impl authenticator::Authenticator for Authenticator {}
