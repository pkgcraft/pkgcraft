use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use itertools::Itertools;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{AsRefStr, Display, EnumString};

use crate::eapi::Eapi;
use crate::macros::{bool_not_equal, cmp_not_equal};
use crate::traits::IntoOwned;
use crate::types::SortedSet;
use crate::Error;

use super::version::{Operator, ParsedVersion, Revision, Version};
use super::{parse, Cpv, UseFlag};

#[repr(C)]
#[derive(
    AsRefStr, Display, EnumString, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
pub enum Blocker {
    #[strum(serialize = "!!")]
    Strong = 1,
    #[strum(serialize = "!")]
    Weak,
}

#[repr(C)]
#[derive(
    AsRefStr, Display, EnumString, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
pub enum SlotOperator {
    #[strum(serialize = "=")]
    Equal = 1,
    #[strum(serialize = "*")]
    Star,
}

#[repr(u32)]
#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum DepField {
    Category = 1,
    Package,
    Blocker,
    Version,
    Slot,
    UseDeps,
    Repo,
}

impl DepField {
    /// Return an iterator consisting of all optional dep fields.
    pub fn optional() -> impl Iterator<Item = Self> {
        use DepField::*;
        [Blocker, Version, Slot, UseDeps, Repo].into_iter()
    }
}

/// Parsed package dep from borrowed input string.
#[derive(Debug, Default)]
pub(crate) struct ParsedDep<'a> {
    pub(crate) category: &'a str,
    pub(crate) package: &'a str,
    pub(crate) blocker: Option<Blocker>,
    pub(crate) version: Option<ParsedVersion<'a>>,
    pub(crate) slot: Option<SlotDep<&'a str>>,
    pub(crate) use_deps: Option<Vec<UseDep<&'a str>>>,
    pub(crate) repo: Option<&'a str>,
}

impl<'a> ParsedDep<'a> {
    /// Used by the parser to inject attributes.
    pub(crate) fn with(
        mut self,
        blocker: Option<Blocker>,
        slot: Option<SlotDep<&'a str>>,
        use_deps: Option<Vec<UseDep<&'a str>>>,
        repo: Option<&'a str>,
    ) -> ParsedDep<'a> {
        self.blocker = blocker;
        self.slot = slot;
        self.use_deps = use_deps;
        self.repo = repo;
        self
    }
}

impl IntoOwned for ParsedDep<'_> {
    type Owned = Dep;

    fn into_owned(self) -> Self::Owned {
        Dep {
            category: self.category.to_string(),
            package: self.package.to_string(),
            blocker: self.blocker,
            version: self.version.into_owned(),
            slot: self.slot.into_owned(),
            use_deps: self
                .use_deps
                .map(|u| u.into_iter().map(|u| u.into_owned()).collect()),
            repo: self.repo.map(|s| s.to_string()),
        }
    }
}

/// Package USE dependency type.
#[repr(C)]
#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum UseDepKind {
    Enabled,             // cat/pkg[opt]
    Disabled,            // cat/pkg[-opt]
    Equal,               // cat/pkg[opt=]
    NotEqual,            // cat/pkg[!opt=]
    EnabledConditional,  // cat/pkg[opt?]
    DisabledConditional, // cat/pkg[!opt?]
}

/// Package USE dependency default when missing.
#[repr(C)]
#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum UseDepDefault {
    Enabled,  // cat/pkg[opt(+)]
    Disabled, // cat/pkg[opt(-)]
}

/// Package USE dependency.
#[derive(DeserializeFromStr, SerializeDisplay, Debug, PartialEq, Eq, Hash, Clone)]
pub struct UseDep<S: UseFlag> {
    pub(crate) kind: UseDepKind,
    pub(crate) flag: S,
    pub(crate) default: Option<UseDepDefault>,
}

impl IntoOwned for UseDep<&str> {
    type Owned = UseDep<String>;

    fn into_owned(self) -> Self::Owned {
        UseDep {
            kind: self.kind,
            flag: self.flag.to_string(),
            default: self.default,
        }
    }
}

impl<S: UseFlag> Ord for UseDep<S> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.flag.cmp(&other.flag)
    }
}

impl<S: UseFlag> PartialOrd for UseDep<S> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<S: UseFlag> fmt::Display for UseDep<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let default = match &self.default {
            Some(UseDepDefault::Enabled) => "(+)",
            Some(UseDepDefault::Disabled) => "(-)",
            None => "",
        };

        let flag = self.flag();
        match &self.kind {
            UseDepKind::Enabled => write!(f, "{flag}{default}"),
            UseDepKind::Disabled => write!(f, "-{flag}{default}"),
            UseDepKind::Equal => write!(f, "{flag}{default}="),
            UseDepKind::NotEqual => write!(f, "!{flag}{default}="),
            UseDepKind::EnabledConditional => write!(f, "{flag}{default}?"),
            UseDepKind::DisabledConditional => write!(f, "!{flag}{default}?"),
        }
    }
}

impl FromStr for UseDep<String> {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        UseDep::new(s)
    }
}

impl FromStr for SortedSet<UseDep<String>> {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        s.split(',').map(UseDep::new).collect()
    }
}

impl<S: UseFlag> UseDep<S> {
    /// Return the USE dependency type.
    pub fn kind(&self) -> UseDepKind {
        self.kind
    }

    /// Return the flag value for the USE dependency.
    pub fn flag(&self) -> &str {
        self.flag.as_ref()
    }

    /// Return the USE dependency default.
    pub fn default(&self) -> Option<UseDepDefault> {
        self.default
    }
}

impl UseDep<String> {
    /// Create a new UseDep from a given string.
    pub fn new(s: &str) -> crate::Result<Self> {
        parse::use_dep(s).into_owned()
    }
}

/// Package slot.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Slot<S> {
    pub(crate) name: S,
}

impl Default for Slot<String> {
    fn default() -> Self {
        Self { name: "0".to_string() }
    }
}

impl IntoOwned for Slot<&str> {
    type Owned = Slot<String>;

    fn into_owned(self) -> Self::Owned {
        Slot { name: self.name.to_string() }
    }
}

impl Slot<String> {
    /// Create a new Slot from a given string.
    pub fn new(s: &str) -> crate::Result<Self> {
        parse::slot(s).into_owned()
    }

    /// Return the main slot value.
    pub fn slot(&self) -> &str {
        let s = self.name.as_str();
        s.split_once('/').map_or(s, |x| x.0)
    }

    /// Return the subslot value if it exists.
    pub fn subslot(&self) -> Option<&str> {
        self.name.split_once('/').map(|x| x.1)
    }

    /// Return the Slot using borrowed values.
    fn as_ref(&self) -> Slot<&str> {
        Slot { name: &self.name }
    }
}

impl PartialEq<Slot<&str>> for Slot<String> {
    fn eq(&self, other: &Slot<&str>) -> bool {
        self.name == other.name
    }
}

impl PartialEq<Slot<String>> for Slot<&str> {
    fn eq(&self, other: &Slot<String>) -> bool {
        other == self
    }
}

impl PartialEq<str> for Slot<String> {
    fn eq(&self, other: &str) -> bool {
        self.name == other
    }
}

impl PartialEq<Slot<String>> for &str {
    fn eq(&self, other: &Slot<String>) -> bool {
        other == *self
    }
}

impl<S: fmt::Display> fmt::Display for Slot<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// Package slot dependency.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct SlotDep<S> {
    pub(crate) slot: Option<Slot<S>>,
    pub(crate) op: Option<SlotOperator>,
}

impl IntoOwned for SlotDep<&str> {
    type Owned = SlotDep<String>;

    fn into_owned(self) -> Self::Owned {
        SlotDep {
            slot: self.slot.into_owned(),
            op: self.op,
        }
    }
}

impl<S: PartialEq + Eq + Ord> Ord for SlotDep<S> {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp_not_equal!(&self.slot, &other.slot);
        self.op.cmp(&other.op)
    }
}

impl<S: PartialEq + Eq + Ord> PartialOrd for SlotDep<S> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq<SlotDep<&str>> for SlotDep<String> {
    fn eq(&self, other: &SlotDep<&str>) -> bool {
        self.slot.as_ref().map(|v| v.as_ref()) == other.slot && self.op == other.op
    }
}

impl PartialEq<SlotDep<String>> for SlotDep<&str> {
    fn eq(&self, other: &SlotDep<String>) -> bool {
        other == self
    }
}

impl<S: fmt::Display> fmt::Display for SlotDep<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match (&self.slot, &self.op) {
            (Some(slot), Some(op)) => write!(f, "{slot}{op}")?,
            (Some(slot), None) => write!(f, "{slot}")?,
            (None, Some(op)) => write!(f, "{op}")?,
            (None, None) => (),
        }
        Ok(())
    }
}

impl SlotDep<String> {
    /// Create a new SlotDep from a given string.
    pub fn new(s: &str) -> crate::Result<Self> {
        parse::slot_dep(s).into_owned()
    }
}

impl FromStr for SlotDep<String> {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        SlotDep::new(s)
    }
}

/// Package dependency.
#[derive(Debug, Clone)]
pub struct Dep {
    category: String,
    package: String,
    blocker: Option<Blocker>,
    version: Option<Version>,
    slot: Option<SlotDep<String>>,
    use_deps: Option<SortedSet<UseDep<String>>>,
    repo: Option<String>,
}

impl PartialEq for Dep {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Dep {}

impl Hash for Dep {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key().hash(state);
    }
}

impl Ord for Dep {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}

impl PartialOrd for Dep {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FromStr for Dep {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Dep::new(s)
    }
}

/// Key type used for implementing various traits, e.g. Eq, Hash, etc.
type DepKey<'a> = (
    &'a str,                               // category
    &'a str,                               // package
    Option<&'a Version>,                   // version
    Option<Blocker>,                       // blocker
    Option<&'a SlotDep<String>>,           // slot
    Option<&'a SortedSet<UseDep<String>>>, // use deps
    Option<&'a str>,                       // repo
);

impl Dep {
    /// Create a new Dep from a given string using the default EAPI.
    pub fn new(s: &str) -> crate::Result<Self> {
        parse::dep(s, Default::default())
    }

    /// Create a new unversioned Dep from a given string.
    pub fn new_cpn(s: &str) -> crate::Result<Self> {
        parse::cpn(s).into_owned()
    }

    /// Potentially create a new Dep, removing the given fields.
    pub fn without<I>(&self, fields: I) -> crate::Result<Cow<'_, Self>>
    where
        I: IntoIterator<Item = DepField>,
    {
        self.modify(fields.into_iter().map(|f| (f, None)))
    }

    /// Potentially create a new Dep, unsetting all optional fields.
    pub fn unversioned(&self) -> Cow<'_, Self> {
        self.modify(DepField::optional().map(|f| (f, None)))
            .unwrap_or_else(|e| panic!("{e}"))
    }

    /// Potentially create a new Dep, unsetting all optional fields except version and setting the
    /// version operator to '='. If the Dep is unversioned a borrowed reference is returned.
    pub fn versioned(&self) -> Cow<'_, Self> {
        let mut dep = self
            .modify(
                DepField::optional()
                    .filter(|f| f != &DepField::Version)
                    .map(|f| (f, None)),
            )
            .unwrap_or_else(|e| panic!("{e}"));

        // set the version operator to '=' if necessary
        if let Some(op) = dep.version().and_then(|v| v.op()) {
            if op != Operator::Equal {
                if let Some(ver) = dep.to_mut().version.as_mut() {
                    ver.op = Some(Operator::Equal);
                }
            }
        }

        dep
    }

    /// Potentially create a new Dep, modifying the given fields and values.
    pub fn modify<'a, I>(&self, values: I) -> crate::Result<Cow<'_, Self>>
    where
        I: IntoIterator<Item = (DepField, Option<&'a str>)>,
    {
        let mut dep = Cow::Borrowed(self);
        for (field, s) in values {
            match field {
                DepField::Category => {
                    if let Some(s) = s {
                        let val = parse::category(s)?;
                        if dep.category != val {
                            dep.to_mut().category = val.to_string();
                        }
                    } else {
                        return Err(Error::InvalidValue("category cannot be unset".to_string()));
                    }
                }
                DepField::Package => {
                    if let Some(s) = s {
                        let val = parse::package(s)?;
                        if dep.package != val {
                            dep.to_mut().package = val.to_string();
                        }
                    } else {
                        return Err(Error::InvalidValue("package cannot be unset".to_string()));
                    }
                }
                DepField::Blocker => {
                    if let Some(s) = s {
                        let val: Blocker = s
                            .parse()
                            .map_err(|_| Error::InvalidValue(format!("invalid blocker: {s}")))?;
                        if !dep.blocker.as_ref().map(|v| v == &val).unwrap_or_default() {
                            dep.to_mut().blocker = Some(val);
                        }
                    } else if self.blocker.is_some() {
                        dep.to_mut().blocker = None;
                    }
                }
                DepField::Version => {
                    if let Some(s) = s {
                        let val = parse::version_with_op(s)?;
                        if !dep.version.as_ref().map(|v| v == &val).unwrap_or_default() {
                            dep.to_mut().version = Some(val);
                        }
                    } else if self.version.is_some() {
                        dep.to_mut().version = None;
                    }
                }
                DepField::Slot => {
                    if let Some(s) = s {
                        let val: SlotDep<String> = s.parse()?;
                        if !dep.slot.as_ref().map(|v| v == &val).unwrap_or_default() {
                            dep.to_mut().slot = Some(val);
                        }
                    } else if self.slot.is_some() {
                        dep.to_mut().slot = None;
                    }
                }
                DepField::UseDeps => {
                    if let Some(s) = s {
                        let val = s.parse()?;
                        if !dep.use_deps.as_ref().map(|v| v == &val).unwrap_or_default() {
                            dep.to_mut().use_deps = Some(val);
                        }
                    } else if self.use_deps.is_some() {
                        dep.to_mut().use_deps = None;
                    }
                }
                DepField::Repo => {
                    if let Some(s) = s {
                        let val = parse::repo(s)?;
                        if !dep.repo.as_ref().map(|v| v == val).unwrap_or_default() {
                            dep.to_mut().repo = Some(val.to_string());
                        }
                    } else if self.repo.is_some() {
                        dep.to_mut().repo = None;
                    }
                }
            }
        }

        Ok(dep)
    }

    /// Verify a string represents a valid package dependency.
    pub fn valid(s: &str, eapi: Option<&'static Eapi>) -> crate::Result<()> {
        parse::dep_str(s, eapi.unwrap_or_default())?;
        Ok(())
    }

    /// Return a package dependency's category.
    pub fn category(&self) -> &str {
        &self.category
    }

    /// Return a package dependency's package.
    pub fn package(&self) -> &str {
        &self.package
    }

    /// Return a package dependency's blocker.
    pub fn blocker(&self) -> Option<Blocker> {
        self.blocker
    }

    /// Return a package dependency's USE flag dependencies.
    pub fn use_deps(&self) -> Option<&SortedSet<UseDep<String>>> {
        self.use_deps.as_ref()
    }

    /// Return a package dependency's version.
    pub fn version(&self) -> Option<&Version> {
        self.version.as_ref()
    }

    /// Return a package dependency's revision.
    pub fn revision(&self) -> Option<&Revision> {
        self.version().and_then(|v| v.revision())
    }

    /// Return a package dependency's version operator.
    pub fn op(&self) -> Option<Operator> {
        self.version().and_then(|v| v.op())
    }

    /// Return the package name and version.
    /// For example, the package dependency "=cat/pkg-1-r2" returns "pkg-1".
    pub fn p(&self) -> String {
        if let Some(ver) = &self.version {
            format!("{}-{}", self.package(), ver.base())
        } else {
            self.package().to_string()
        }
    }

    /// Return the package name, version, and revision.
    /// For example, the package dependency "=cat/pkg-1-r2" returns "pkg-1-r2".
    pub fn pf(&self) -> String {
        if self.version.is_some() {
            format!("{}-{}", self.package(), self.pvr())
        } else {
            self.package().to_string()
        }
    }

    /// Return the package dependency's revision.
    /// For example, the package dependency "=cat/pkg-1-r2" returns "r2".
    pub fn pr(&self) -> String {
        self.version()
            .map(|v| format!("r{}", v.revision().map(|r| r.as_ref()).unwrap_or("0")))
            .unwrap_or_default()
    }

    /// Return the package dependency's version.
    /// For example, the package dependency "=cat/pkg-1-r2" returns "1".
    pub fn pv(&self) -> String {
        self.version()
            .map(|v| v.base().to_string())
            .unwrap_or_default()
    }

    /// Return the package dependency's version and revision.
    /// For example, the package dependency "=cat/pkg-1-r2" returns "1-r2".
    pub fn pvr(&self) -> String {
        self.version().map(|v| v.without_op()).unwrap_or_default()
    }

    /// Return the package dependency's category and package.
    /// For example, the package dependency "=cat/pkg-1-r2" returns "cat/pkg".
    pub fn cpn(&self) -> String {
        format!("{}/{}", self.category, self.package)
    }

    /// Return the package dependency's category, package, version, and revision.
    /// For example, the package dependency "=cat/pkg-1-r2" returns "cat/pkg-1-r2".
    pub fn cpv(&self) -> String {
        if let Some(ver) = &self.version {
            format!("{}/{}-{}", self.category, self.package, ver.without_op())
        } else {
            self.cpn()
        }
    }

    /// Return a package dependency's slot.
    pub fn slot(&self) -> Option<&str> {
        self.slot
            .as_ref()
            .and_then(|s| s.slot.as_ref())
            .map(|s| s.slot())
    }

    /// Return a package dependency's subslot.
    pub fn subslot(&self) -> Option<&str> {
        self.slot
            .as_ref()
            .and_then(|s| s.slot.as_ref())
            .and_then(|s| s.subslot())
    }

    /// Return a package dependency's slot operator.
    pub fn slot_op(&self) -> Option<SlotOperator> {
        self.slot.as_ref().and_then(|s| s.op)
    }

    /// Return a package dependency's repository.
    pub fn repo(&self) -> Option<&str> {
        self.repo.as_deref()
    }

    /// Return a key value used to implement various traits, e.g. Eq, Hash, etc.
    fn key(&self) -> DepKey {
        (
            self.category(),
            self.package(),
            self.version(),
            self.blocker(),
            self.slot.as_ref(),
            self.use_deps(),
            self.repo(),
        )
    }
}

impl fmt::Display for Dep {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // append blocker
        if let Some(blocker) = self.blocker {
            write!(f, "{blocker}")?;
        }

        // append version operator with cpv
        let cpv = self.cpv();
        use Operator::*;
        match self.version().and_then(|v| v.op()) {
            None => write!(f, "{cpv}")?,
            Some(Less) => write!(f, "<{cpv}")?,
            Some(LessOrEqual) => write!(f, "<={cpv}")?,
            Some(Equal) => write!(f, "={cpv}")?,
            Some(EqualGlob) => write!(f, "={cpv}*")?,
            Some(Approximate) => write!(f, "~{cpv}")?,
            Some(GreaterOrEqual) => write!(f, ">={cpv}")?,
            Some(Greater) => write!(f, ">{cpv}")?,
        }

        // append slot
        if let Some(slot) = &self.slot {
            write!(f, ":{slot}")?;
        }

        // append repo
        if let Some(repo) = &self.repo {
            write!(f, "::{repo}")?;
        }

        // append use deps
        if let Some(x) = &self.use_deps {
            write!(f, "[{}]", x.iter().join(","))?;
        }

        Ok(())
    }
}

/// Determine if two objects intersect.
pub trait Intersects<T> {
    fn intersects(&self, obj: &T) -> bool;
}

/// Determine if a package dependency intersects with a Cpv.
impl Intersects<Cpv> for Dep {
    fn intersects(&self, other: &Cpv) -> bool {
        bool_not_equal!(&self.category(), &other.category());
        bool_not_equal!(&self.package(), &other.package());
        self.version()
            .map(|v| v.intersects(other.version()))
            .unwrap_or(true)
    }
}

/// Determine if two package dependencies intersect ignoring blockers.
impl Intersects<Dep> for Dep {
    fn intersects(&self, other: &Dep) -> bool {
        bool_not_equal!(&self.category(), &other.category());
        bool_not_equal!(&self.package(), &other.package());

        if let (Some(x), Some(y)) = (self.slot(), other.slot()) {
            bool_not_equal!(x, y);
        }

        if let (Some(x), Some(y)) = (self.subslot(), other.subslot()) {
            bool_not_equal!(x, y);
        }

        if let (Some(x), Some(y)) = (self.use_deps(), other.use_deps()) {
            let mut use_map = HashMap::<_, HashSet<_>>::new();
            for u in x.symmetric_difference(y) {
                use_map.entry(&u.flag).or_default().insert(&u.kind);
            }
            for kinds in use_map.values() {
                if kinds.contains(&UseDepKind::Disabled) && kinds.contains(&UseDepKind::Enabled) {
                    return false;
                }
            }
        }

        if let (Some(x), Some(y)) = (self.repo(), other.repo()) {
            bool_not_equal!(x, y);
        }

        if let (Some(x), Some(y)) = (self.version(), other.version()) {
            x.intersects(y)
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use indexmap::IndexSet;

    use crate::dep::CpvOrDep;
    use crate::eapi::{self, EAPIS};
    use crate::test::TEST_DATA;
    use crate::utils::hash;

    use super::*;

    #[test]
    fn new_and_valid() {
        // invalid
        for s in &TEST_DATA.dep_toml.invalid {
            for eapi in &*EAPIS {
                let result = Dep::valid(s, Some(*eapi));
                assert!(result.is_err(), "{s:?} is valid for EAPI={eapi}");
                let result = eapi.dep(s);
                assert!(result.is_err(), "{s:?} didn't fail for EAPI={eapi}");
            }
        }

        // valid
        for e in &TEST_DATA.dep_toml.valid {
            let s = e.dep.as_str();
            let passing_eapis: IndexSet<_> = eapi::range(&e.eapis).unwrap().collect();
            for eapi in &passing_eapis {
                let result = Dep::valid(s, Some(*eapi));
                assert!(result.is_ok(), "{s:?} isn't valid for EAPI={eapi}");
                let result = eapi.dep(s);
                assert!(result.is_ok(), "{s:?} failed for EAPI={eapi}");
                let d = result.unwrap();
                assert_eq!(d.category(), e.category, "{s:?} failed for EAPI={eapi}");
                assert_eq!(d.package(), e.package, "{s:?} failed for EAPI={eapi}");
                assert_eq!(d.blocker(), e.blocker, "{s:?} failed for EAPI={eapi}");
                assert_eq!(d.version(), e.version.as_ref(), "{s:?} failed for EAPI={eapi}");
                assert_eq!(d.revision(), e.revision.as_ref(), "{s:?} failed for EAPI={eapi}");
                assert_eq!(d.slot(), e.slot.as_deref(), "{s:?} failed for EAPI={eapi}");
                assert_eq!(d.subslot(), e.subslot.as_deref(), "{s:?} failed for EAPI={eapi}");
                assert_eq!(d.slot_op(), e.slot_op, "{s:?} failed for EAPI={eapi}");
                assert_eq!(d.use_deps(), e.use_deps.as_ref(), "{s:?} failed for EAPI={eapi}");
                assert_eq!(d.to_string(), s, "{s:?} failed for EAPI={eapi}");
            }
            for eapi in EAPIS.difference(&passing_eapis) {
                let result = Dep::valid(s, Some(*eapi));
                assert!(result.is_err(), "{s:?} is valid for EAPI={eapi}");
                let result = eapi.dep(s);
                assert!(result.is_err(), "{s:?} didn't fail for EAPI={eapi}");
            }
        }
    }

    #[test]
    fn to_string() {
        for s in [
            "cat/pkg",
            "<cat/pkg-4",
            "<=cat/pkg-4-r1",
            "=cat/pkg-4-r0",
            "=cat/pkg-4-r01",
            "=cat/pkg-4*",
            "~cat/pkg-4",
            ">=cat/pkg-r1-2-r3",
            ">cat/pkg-4-r1:0=",
            ">cat/pkg-4-r1:0/2=[use]",
            ">cat/pkg-4-r1:0/2=::repo[use]",
            "!cat/pkg",
            "!!<cat/pkg-4",
        ] {
            let dep: Dep = s.parse().unwrap();
            assert_eq!(dep.to_string(), s);
        }
    }

    #[test]
    fn cpn() {
        for (s, key) in [
            ("cat/pkg", "cat/pkg"),
            ("<cat/pkg-4", "cat/pkg"),
            ("<=cat/pkg-4-r1", "cat/pkg"),
            ("=cat/pkg-4", "cat/pkg"),
            ("=cat/pkg-4*", "cat/pkg"),
            ("~cat/pkg-4", "cat/pkg"),
            (">=cat/pkg-r1-2-r3", "cat/pkg-r1"),
            (">cat/pkg-4-r1:0=", "cat/pkg"),
        ] {
            let dep: Dep = s.parse().unwrap();
            assert_eq!(dep.cpn(), key);
        }
    }

    #[test]
    fn version() {
        for (s, version) in [
            ("cat/pkg", None),
            ("<cat/pkg-4", Some("<4")),
            ("<=cat/pkg-4-r1", Some("<=4-r1")),
            ("=cat/pkg-4", Some("=4")),
            ("=cat/pkg-4*", Some("=4*")),
            ("~cat/pkg-4", Some("~4")),
            (">=cat/pkg-r1-2-r3", Some(">=2-r3")),
            (">cat/pkg-4-r1:0=", Some(">4-r1")),
        ] {
            let dep: Dep = s.parse().unwrap();
            let version = version.map(|s| parse::version_with_op(s).unwrap());
            assert_eq!(dep.version(), version.as_ref());
        }
    }

    #[test]
    fn revision() {
        for (s, rev_str) in [
            ("cat/pkg", None),
            ("<cat/pkg-4", None),
            ("=cat/pkg-4-r0", Some("0")),
            ("<=cat/pkg-4-r1", Some("1")),
            (">=cat/pkg-r1-2-r3", Some("3")),
            (">cat/pkg-4-r1:0=", Some("1")),
        ] {
            let dep: Dep = s.parse().unwrap();
            let rev = rev_str.map(|s| s.parse().unwrap());
            assert_eq!(dep.revision(), rev.as_ref(), "{s} failed");
        }
    }

    #[test]
    fn op() {
        for (s, op) in [
            ("cat/pkg", None),
            ("<cat/pkg-4", Some(Operator::Less)),
            ("<=cat/pkg-4-r1", Some(Operator::LessOrEqual)),
            ("=cat/pkg-4", Some(Operator::Equal)),
            ("=cat/pkg-4*", Some(Operator::EqualGlob)),
            ("~cat/pkg-4", Some(Operator::Approximate)),
            (">=cat/pkg-r1-2-r3", Some(Operator::GreaterOrEqual)),
            (">cat/pkg-4-r1:0=", Some(Operator::Greater)),
        ] {
            let dep: Dep = s.parse().unwrap();
            assert_eq!(dep.op(), op);
        }
    }

    #[test]
    fn cpv() {
        for (s, cpv) in [
            ("cat/pkg", "cat/pkg"),
            ("<cat/pkg-4", "cat/pkg-4"),
            ("<=cat/pkg-4-r1", "cat/pkg-4-r1"),
            ("=cat/pkg-4", "cat/pkg-4"),
            ("=cat/pkg-4*", "cat/pkg-4"),
            ("~cat/pkg-4", "cat/pkg-4"),
            (">=cat/pkg-r1-2-r3", "cat/pkg-r1-2-r3"),
            (">cat/pkg-4-r1:0=", "cat/pkg-4-r1"),
        ] {
            let dep: Dep = s.parse().unwrap();
            assert_eq!(dep.cpv(), cpv);
        }
    }

    #[test]
    fn cmp() {
        let op_map: HashMap<_, _> =
            [("<", Ordering::Less), ("==", Ordering::Equal), (">", Ordering::Greater)]
                .into_iter()
                .collect();

        for (expr, (s1, op, s2)) in TEST_DATA.dep_toml.compares() {
            let dep1: Dep = s1.parse().unwrap();
            let dep2: Dep = s2.parse().unwrap();
            if op == "!=" {
                assert_ne!(dep1, dep2, "failed comparing: {expr}");
                assert_ne!(dep2, dep1, "failed comparing: {expr}");
            } else {
                let op = op_map[op];
                assert_eq!(dep1.cmp(&dep2), op, "failed comparing: {expr}");
                assert_eq!(dep2.cmp(&dep1), op.reverse(), "failed comparing inverted: {expr}");

                // verify the following property holds since both Hash and Eq are implemented:
                // k1 == k2 -> hash(k1) == hash(k2)
                if op == Ordering::Equal {
                    assert_eq!(hash(dep1), hash(dep2), "failed hash: {expr}");
                }
            }
        }
    }

    #[test]
    fn intersects() {
        // inject version intersects data from version.toml into Dep objects
        let dep = Dep::new("a/b").unwrap();
        for d in &TEST_DATA.version_toml.intersects {
            // test intersections between all pairs of distinct values
            let permutations = d
                .vals
                .iter()
                .map(|s| s.as_str())
                .permutations(2)
                .map(|val| val.into_iter().collect_tuple().unwrap());
            for (s1, s2) in permutations {
                let (mut dep1, mut dep2) = (dep.clone(), dep.clone());
                dep1.version = Some(s1.parse().unwrap());
                dep2.version = Some(s2.parse().unwrap());

                // self intersection
                assert!(dep1.intersects(&dep1), "{dep1} doesn't intersect itself");
                assert!(dep2.intersects(&dep2), "{dep2} doesn't intersect itself");

                // intersects depending on status
                if d.status {
                    assert!(dep1.intersects(&dep2), "{dep1} doesn't intersect {dep2}");
                } else {
                    assert!(!dep1.intersects(&dep2), "{dep1} intersects {dep2}");
                }
            }
        }

        for d in &TEST_DATA.dep_toml.intersects {
            // test intersections between all pairs of distinct values
            let permutations = d
                .vals
                .iter()
                .map(|s| s.as_str())
                .permutations(2)
                .map(|val| val.into_iter().collect_tuple().unwrap());
            for (s1, s2) in permutations {
                let obj1: CpvOrDep = s1.parse().unwrap();
                let obj2: CpvOrDep = s2.parse().unwrap();

                // self intersection
                assert!(obj1.intersects(&obj1), "{obj1} doesn't intersect {obj1}");
                assert!(obj2.intersects(&obj2), "{obj2} doesn't intersect {obj2}");

                // intersects depending on status
                if d.status {
                    assert!(obj1.intersects(&obj2), "{obj1} doesn't intersect {obj2}");
                } else {
                    assert!(!obj1.intersects(&obj2), "{obj1} intersects {obj2}");
                }
            }
        }
    }

    #[test]
    fn without() {
        let dep = Dep::new("!!>=cat/pkg-1.2-r3:4/5=::repo[a,b]").unwrap();

        for (field, expected) in [
            (DepField::Blocker, ">=cat/pkg-1.2-r3:4/5=::repo[a,b]"),
            (DepField::Slot, "!!>=cat/pkg-1.2-r3::repo[a,b]"),
            (DepField::Version, "!!cat/pkg:4/5=::repo[a,b]"),
            (DepField::UseDeps, "!!>=cat/pkg-1.2-r3:4/5=::repo"),
            (DepField::Repo, "!!>=cat/pkg-1.2-r3:4/5=[a,b]"),
        ] {
            let d = dep.without([field]).unwrap();
            let s = d.to_string();
            assert_eq!(&s, expected);
            assert_eq!(d.as_ref(), &Dep::new(&s).unwrap());
        }

        // remove all fields
        let d = dep.without(DepField::optional()).unwrap();
        let s = d.to_string();
        assert_eq!(&s, "cat/pkg");
        assert_eq!(d.as_ref(), &Dep::new(&s).unwrap());

        // verify all combinations of dep fields create valid deps
        for vals in DepField::optional().powerset() {
            let d = dep.without(vals).unwrap();
            let s = d.to_string();
            assert_eq!(d.as_ref(), &Dep::new(&s).unwrap());
        }

        // no changes returns a borrowed dep
        let dep = Dep::new("cat/pkg").unwrap();
        matches!(dep.without([DepField::Version]).unwrap(), Cow::Borrowed(_));
    }

    #[test]
    fn modify() {
        let dep = Dep::new("!!>=cat/pkg-1.2-r3:4::repo[a,b]").unwrap();

        // modify single fields
        for (field, val, expected) in [
            (DepField::Category, "a", "!!>=a/pkg-1.2-r3:4::repo[a,b]"),
            (DepField::Package, "b", "!!>=cat/b-1.2-r3:4::repo[a,b]"),
            (DepField::Blocker, "!", "!>=cat/pkg-1.2-r3:4::repo[a,b]"),
            (DepField::Slot, "1", "!!>=cat/pkg-1.2-r3:1::repo[a,b]"),
            (DepField::Slot, "1/2", "!!>=cat/pkg-1.2-r3:1/2::repo[a,b]"),
            (DepField::Slot, "1/2=", "!!>=cat/pkg-1.2-r3:1/2=::repo[a,b]"),
            (DepField::Slot, "*", "!!>=cat/pkg-1.2-r3:*::repo[a,b]"),
            (DepField::Slot, "=", "!!>=cat/pkg-1.2-r3:=::repo[a,b]"),
            (DepField::Version, "<0", "!!<cat/pkg-0:4::repo[a,b]"),
            (DepField::UseDeps, "x,y,z", "!!>=cat/pkg-1.2-r3:4::repo[x,y,z]"),
            (DepField::Repo, "test", "!!>=cat/pkg-1.2-r3:4::test[a,b]"),
        ] {
            let d = dep.modify([(field, Some(val))]).unwrap();
            let s = d.to_string();
            assert_eq!(&s, expected);
            assert_eq!(d.as_ref(), &Dep::new(&s).unwrap());
        }

        // remove all optional fields
        let d = dep.modify(DepField::optional().map(|f| (f, None))).unwrap();
        assert_eq!(d.to_string(), "cat/pkg");

        // multiple modifications
        let d = dep
            .modify([(DepField::Repo, None), (DepField::Version, Some("~5"))])
            .unwrap();
        assert_eq!(d.to_string(), "!!~cat/pkg-5:4[a,b]");

        // multiple modifications to the same field
        let d = dep
            .modify([(DepField::Repo, None), (DepField::Repo, Some("r2"))])
            .unwrap();
        assert_eq!(d.to_string(), "!!>=cat/pkg-1.2-r3:4::r2[a,b]");

        // removing non-optional fields fails
        assert!(dep.modify([(DepField::Category, None)]).is_err());
        assert!(dep.modify([(DepField::Package, None)]).is_err());

        // verify all combinations of dep field modifications create valid deps
        let fields = [
            (DepField::Category, Some("a")),
            (DepField::Package, Some("b")),
            (DepField::Blocker, Some("!")),
            (DepField::Slot, Some("1/2=")),
            (DepField::Version, Some("<0")),
            (DepField::UseDeps, Some("x,y,z")),
            (DepField::Repo, Some("test")),
        ];
        for vals in fields.into_iter().powerset() {
            let d = dep.modify(vals).unwrap();
            let s = d.to_string();
            assert_eq!(d.as_ref(), &Dep::new(&s).unwrap());
        }

        // verify all combinations of removing optional dep fields create valid deps
        for vals in DepField::optional().powerset() {
            let d = dep.modify(vals.into_iter().map(|f| (f, None))).unwrap();
            let s = d.to_string();
            assert_eq!(d.as_ref(), &Dep::new(&s).unwrap());
        }

        // invalid values
        assert!(dep.modify([(DepField::Category, Some("+"))]).is_err());
        assert!(dep.modify([(DepField::Package, Some("+"))]).is_err());
        assert!(dep.modify([(DepField::Blocker, Some("+"))]).is_err());
        assert!(dep.modify([(DepField::Slot, Some("+"))]).is_err());
        assert!(dep.modify([(DepField::Version, Some("+"))]).is_err());
        assert!(dep.modify([(DepField::UseDeps, Some("+"))]).is_err());
        assert!(dep.modify([(DepField::Repo, Some("+"))]).is_err());

        // no changes returns a borrowed dep
        let dep = Dep::new("cat/pkg").unwrap();
        matches!(dep.modify([(DepField::Category, Some("cat"))]).unwrap(), Cow::Borrowed(_));
        matches!(dep.modify([(DepField::Version, None)]).unwrap(), Cow::Borrowed(_));
    }

    #[test]
    fn sorting() {
        for d in &TEST_DATA.dep_toml.sorting {
            let mut reversed: Vec<Dep> =
                d.sorted.iter().map(|s| s.parse().unwrap()).rev().collect();
            reversed.sort();
            let mut sorted: Vec<_> = reversed.iter().map(|x| x.to_string()).collect();
            if d.equal {
                // equal deps aren't sorted so reversing should restore the original order
                sorted = sorted.into_iter().rev().collect();
            }
            assert_eq!(&sorted, &d.sorted);
        }
    }

    #[test]
    fn hashing() {
        for d in &TEST_DATA.version_toml.hashing {
            let set: HashSet<Dep> = d
                .versions
                .iter()
                .map(|s| format!("=cat/pkg-{s}").parse().unwrap())
                .collect();
            if d.equal {
                assert_eq!(set.len(), 1, "failed hashing deps: {set:?}");
            } else {
                assert_eq!(set.len(), d.versions.len(), "failed hashing deps: {set:?}");
            }
        }
    }
}
