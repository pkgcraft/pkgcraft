pub(crate) mod cpv;
pub mod parse;
pub(crate) mod pkg;
pub mod spec;
pub mod version;

pub use cpv::{Cpv, CpvOrDep};
pub use pkg::{Blocker, Dep, DepField, DepFields, Intersects, SlotOperator};
pub use spec::{
    Conditionals, DepSet, DepSpec, Evaluate, EvaluateForce, Flatten, IntoOwned, Recursive, Uri,
    UseFlag,
};
pub use version::{Operator, Revision, Version};
