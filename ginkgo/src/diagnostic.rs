use std::{error::Error, fmt};

pub type Result<T> = core::result::Result<T, BoxedDiagnostic>;

#[derive(Debug)]
pub struct BoxedDiagnostic(Box<dyn Diagnostic>);

impl fmt::Display for BoxedDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub trait Diagnostic: Error {}

impl<T> From<T> for BoxedDiagnostic
where
    T: Diagnostic + 'static,
{
    fn from(value: T) -> Self {
        BoxedDiagnostic(Box::new(value))
    }
}
