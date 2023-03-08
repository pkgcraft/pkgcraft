pub(crate) mod cpv;
pub mod parse;
pub(crate) mod pkg;
pub mod spec;
pub mod version;

pub use cpv::{Cpv, CpvOrDep, TryIntoCpv};
pub use pkg::{Blocker, Dep, Intersects, SlotOperator};
pub use spec::{DepSet, DepSpec, Flatten, Recursive, Uri};
pub use version::{Operator, Revision, Version};
