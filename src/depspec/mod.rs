mod license;

#[derive(Debug, PartialEq)]
pub enum DepSpec {
    Names(Vec<String>),
    AllOf(Box<DepSpec>),
    AnyOf(Box<DepSpec>),
    ConditionalUse(String, Box<DepSpec>),
}
