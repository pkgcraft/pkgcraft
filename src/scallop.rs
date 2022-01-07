use crate::Error;

pub mod builtins;

impl From<Error> for scallop::Error {
    fn from(e: Error) -> Self {
        scallop::Error::new(e.to_string())
    }
}
