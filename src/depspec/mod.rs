mod license;
mod required_use;

#[derive(Debug, PartialEq)]
pub enum DepSpec {
    Names(Vec<String>),
    AllOf(Box<DepSpec>),
    AnyOf(Box<DepSpec>),
    ExactlyOneOf(Box<DepSpec>), // REQUIRED_USE only
    AtMostOneOf(Box<DepSpec>), // REQUIRED_USE only
    ConditionalUse(String, Box<DepSpec>),
}
