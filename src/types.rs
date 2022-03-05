use walkdir::DirEntry;

pub(crate) type WalkDirFilter = fn(&DirEntry) -> bool;
pub(crate) const NO_WALKDIR_FILTER: Option<WalkDirFilter> = None::<WalkDirFilter>;
