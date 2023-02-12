pub mod parse;
pub(crate) mod pkg;
pub(crate) mod set;
pub(crate) mod version;

pub use pkg::{Blocker, PkgDep, SlotOperator};
pub use set::{
    Dep, DepSet, DepSetIntoIter, DepSetIntoIterFlatten, DepSetIntoIterRecursive, DepSetIter,
    DepSetIterFlatten, DepSetIterRecursive, IntoIteratorDepSet, Uri,
};
pub use version::{Operator, Revision, Version};
