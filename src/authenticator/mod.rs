pub trait Authenticator {}

impl Authenticator for Box<dyn Authenticator> {}

mod insecure;
pub use insecure::Authenticator as Insecure;
