pub mod parse;
pub(crate) mod pkg;
pub mod spec;
pub mod version;

pub use pkg::{Blocker, Cpv, CpvOrDep, Dep, Intersects, SlotOperator, TryIntoCpv};
pub use spec::{DepSet, DepSpec, Flatten, Recursive, Uri};
pub use version::{Operator, Revision, Version};
