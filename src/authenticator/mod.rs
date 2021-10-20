pub trait Authenticator {
    type UserState;
}

mod insecure;
pub use insecure::Authenticator as Insecure;
