use walkdir::DirEntry;

// None value coerced to a directory filtering predicate function pointer for use with
// Option-wrapped closure parameter generics.
type WalkDirFilter = fn(&DirEntry) -> bool;
pub(crate) const NO_WALKDIR_FILTER: Option<WalkDirFilter> = None::<WalkDirFilter>;
