use std::error::Error;

// TODO: miette seems to use a concrete type here that stores the trait object?
pub type Result<T> = core::result::Result<T, BoxedDiagnostic>;

#[derive(Debug)]
pub struct BoxedDiagnostic(Box<dyn Diagnostic>);

pub trait Diagnostic: Error {}

impl<T> From<T> for BoxedDiagnostic
where
    T: Diagnostic + 'static,
{
    fn from(value: T) -> Self {
        BoxedDiagnostic(Box::new(value))
    }
}
