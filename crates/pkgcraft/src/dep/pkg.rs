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
use crate::macros::{
    bool_not_equal, cmp_not_equal, equivalent, partial_cmp_opt_not_equal,
    partial_cmp_opt_not_equal_opt,
};
use crate::traits::{Intersects, IntoOwned, ToRef};
use crate::types::SortedSet;
use crate::Error;

use super::use_dep::{UseDep, UseDepKind};
use super::version::{Operator, Revision, Version};
use super::{parse, Cpn, Cpv, Stringable};

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
    SlotDep,
    UseDeps,
    Repo,
}

impl DepField {
    /// Return an iterator consisting of all optional dep fields.
    pub fn optional() -> impl Iterator<Item = Self> {
        use DepField::*;
        [Blocker, Version, SlotDep, UseDeps, Repo].into_iter()
    }
}

/// Package slot.
#[derive(Debug, Eq, Ord, Hash, Clone)]
pub struct Slot<S: Stringable> {
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

impl<'a, S: Stringable> ToRef<'a> for Slot<S> {
    type Ref = Slot<&'a str>;

    fn to_ref(&'a self) -> Self::Ref {
        Slot { name: self.name.as_ref() }
    }
}

impl Slot<String> {
    /// Create a new Slot from a given string.
    pub fn try_new(s: &str) -> crate::Result<Self> {
        parse::slot(s).into_owned()
    }
}

impl<S: Stringable> Slot<S> {
    /// Return the main slot value.
    pub fn slot(&self) -> &str {
        let s = self.name.as_ref();
        s.split_once('/').map_or(s, |x| x.0)
    }

    /// Return the subslot value if it exists.
    pub fn subslot(&self) -> Option<&str> {
        self.name.as_ref().split_once('/').map(|x| x.1)
    }
}

impl<S1: Stringable, S2: Stringable> PartialEq<Slot<S1>> for Slot<S2> {
    fn eq(&self, other: &Slot<S1>) -> bool {
        self.name.as_ref() == other.name.as_ref()
    }
}

impl<S1: Stringable, S2: Stringable> PartialOrd<Slot<S1>> for Slot<S2> {
    fn partial_cmp(&self, other: &Slot<S1>) -> Option<Ordering> {
        Some(self.name.as_ref().cmp(other.name.as_ref()))
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

impl<S: Stringable> fmt::Display for Slot<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// Package slot dependency.
#[derive(Debug, Ord, Eq, Hash, Clone)]
pub struct SlotDep<S: Stringable> {
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

impl<S1: Stringable, S2: Stringable> PartialEq<SlotDep<S1>> for SlotDep<S2> {
    fn eq(&self, other: &SlotDep<S1>) -> bool {
        self.slot.to_ref() == other.slot.to_ref() && self.op == other.op
    }
}

impl<S1: Stringable, S2: Stringable> PartialOrd<SlotDep<S1>> for SlotDep<S2> {
    fn partial_cmp(&self, other: &SlotDep<S1>) -> Option<Ordering> {
        partial_cmp_opt_not_equal_opt!(&self.slot, &other.slot);
        Some(self.op.cmp(&other.op))
    }
}

impl<S: Stringable> fmt::Display for SlotDep<S> {
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
    pub fn try_new(s: &str) -> crate::Result<Self> {
        parse::slot_dep(s).into_owned()
    }
}

impl<S: Stringable> SlotDep<S> {
    /// Return the slot.
    pub fn slot(&self) -> Option<&Slot<S>> {
        self.slot.as_ref()
    }

    /// Return the slot operator.
    pub fn op(&self) -> Option<SlotOperator> {
        self.op
    }
}

impl FromStr for SlotDep<String> {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::try_new(s)
    }
}

/// Package dependency.
#[derive(SerializeDisplay, DeserializeFromStr, Debug, Clone)]
pub struct Dep<S: Stringable> {
    pub(crate) cpn: Cpn<S>,
    pub(crate) blocker: Option<Blocker>,
    pub(crate) version: Option<Version<S>>,
    pub(crate) slot_dep: Option<SlotDep<S>>,
    pub(crate) use_deps: Option<SortedSet<UseDep<S>>>,
    pub(crate) repo: Option<S>,
}

impl<'a> Dep<&'a str> {
    /// Used by the parser to inject attributes.
    pub(crate) fn with(
        mut self,
        blocker: Option<Blocker>,
        slot_dep: Option<SlotDep<&'a str>>,
        use_deps: Option<Vec<UseDep<&'a str>>>,
        repo: Option<&'a str>,
    ) -> Self {
        self.blocker = blocker;
        self.slot_dep = slot_dep;
        self.use_deps = use_deps.map(|u| u.into_iter().collect());
        self.repo = repo;
        self
    }
}

impl IntoOwned for Dep<&str> {
    type Owned = Dep<String>;

    fn into_owned(self) -> Self::Owned {
        Dep {
            cpn: self.cpn.into_owned(),
            blocker: self.blocker,
            version: self.version.into_owned(),
            slot_dep: self.slot_dep.into_owned(),
            use_deps: self
                .use_deps
                .map(|u| u.into_iter().map(|u| u.into_owned()).collect()),
            repo: self.repo.map(|s| s.to_string()),
        }
    }
}

impl<S: Stringable> From<Cpn<S>> for Dep<S> {
    fn from(cpn: Cpn<S>) -> Self {
        Self {
            cpn,
            blocker: None,
            version: None,
            slot_dep: None,
            use_deps: None,
            repo: None,
        }
    }
}

impl<S: Stringable> From<Dep<S>> for Cpn<S> {
    fn from(dep: Dep<S>) -> Self {
        dep.cpn.clone()
    }
}

impl<S1: Stringable, S2: Stringable> PartialEq<Dep<S1>> for Dep<S2> {
    fn eq(&self, other: &Dep<S1>) -> bool {
        cmp(self, other) == Ordering::Equal
    }
}

impl<S1: Stringable, S2: Stringable> PartialEq<Cow<'_, Dep<S1>>> for Dep<S2> {
    fn eq(&self, other: &Cow<'_, Dep<S1>>) -> bool {
        cmp(self, other) == Ordering::Equal
    }
}

impl<S1: Stringable, S2: Stringable> PartialEq<Dep<S1>> for Cow<'_, Dep<S2>> {
    fn eq(&self, other: &Dep<S1>) -> bool {
        cmp(self, other) == Ordering::Equal
    }
}

impl<S: Stringable> Eq for Dep<S> {}

impl<S: Stringable> Hash for Dep<S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key().hash(state);
    }
}

impl<S: Stringable> Ord for Dep<S> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}

impl<S1: Stringable, S2: Stringable> PartialOrd<Dep<S1>> for Dep<S2> {
    fn partial_cmp(&self, other: &Dep<S1>) -> Option<Ordering> {
        Some(cmp(self, other))
    }
}

impl<S1: Stringable, S2: Stringable> PartialOrd<Cow<'_, Dep<S1>>> for Dep<S2> {
    fn partial_cmp(&self, other: &Cow<'_, Dep<S1>>) -> Option<Ordering> {
        Some(cmp(self, other))
    }
}

impl<S1: Stringable, S2: Stringable> PartialOrd<Dep<S1>> for Cow<'_, Dep<S2>> {
    fn partial_cmp(&self, other: &Dep<S1>) -> Option<Ordering> {
        Some(cmp(self, other))
    }
}

impl FromStr for Dep<String> {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::try_new(s)
    }
}

/// Key type used for implementing various traits, e.g. Eq, Hash, etc.
type DepKey<'a, S> = (
    &'a Cpn<S>,                       // unversioned dep
    Option<&'a Version<S>>,           // version
    Option<Blocker>,                  // blocker
    Option<&'a SlotDep<S>>,           // slot
    Option<&'a SortedSet<UseDep<S>>>, // use deps
    Option<&'a str>,                  // repo
);

impl Dep<String> {
    /// Create a new Dep from a given string using the default EAPI.
    pub fn try_new<S: AsRef<str>>(s: S) -> crate::Result<Self> {
        parse::dep(s.as_ref(), Default::default())
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

    /// Potentially create a new Dep without use dependencies.
    pub fn no_use_deps(&self) -> Cow<'_, Self> {
        self.without([DepField::UseDeps])
            .unwrap_or_else(|e| panic!("{e}"))
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
                        if dep.cpn.category != val {
                            dep.to_mut().cpn.category = val.to_string();
                        }
                    } else {
                        return Err(Error::InvalidValue("category cannot be unset".to_string()));
                    }
                }
                DepField::Package => {
                    if let Some(s) = s {
                        let val = parse::package(s)?;
                        if dep.cpn.package != val {
                            dep.to_mut().cpn.package = val.to_string();
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
                        let val = parse::version_with_op(s).into_owned()?;
                        if !dep.version.as_ref().map(|v| v == &val).unwrap_or_default() {
                            dep.to_mut().version = Some(val);
                        }
                    } else if self.version.is_some() {
                        dep.to_mut().version = None;
                    }
                }
                DepField::SlotDep => {
                    if let Some(s) = s {
                        let val = SlotDep::try_new(s)?;
                        if !dep.slot_dep.as_ref().map(|v| v == &val).unwrap_or_default() {
                            dep.to_mut().slot_dep = Some(val);
                        }
                    } else if self.slot_dep.is_some() {
                        dep.to_mut().slot_dep = None;
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
}

impl<'a> Dep<&'a str> {
    /// Create a borrowed [`Dep`] from a given string.
    pub fn parse(s: &'a str, eapi: Option<&'static Eapi>) -> crate::Result<Self> {
        parse::dep_str(s, eapi.unwrap_or_default())
    }
}

impl<S: Stringable> Dep<S> {
    /// Return a package dependency's [`Cpn`].
    pub fn cpn(&self) -> &Cpn<S> {
        &self.cpn
    }

    /// Return a package dependency's category.
    pub fn category(&self) -> &str {
        self.cpn.category()
    }

    /// Return a package dependency's package.
    pub fn package(&self) -> &str {
        self.cpn.package()
    }

    /// Return a package dependency's blocker.
    pub fn blocker(&self) -> Option<Blocker> {
        self.blocker
    }

    /// Return a package dependency's USE flag dependencies.
    pub fn use_deps(&self) -> Option<&SortedSet<UseDep<S>>> {
        self.use_deps.as_ref()
    }

    /// Return a package dependency's version.
    pub fn version(&self) -> Option<&Version<S>> {
        self.version.as_ref()
    }

    /// Return a package dependency's revision.
    pub fn revision(&self) -> Option<&Revision<S>> {
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
            .map(|v| format!("r{}", v.revision().map(|r| r.as_str()).unwrap_or("0")))
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

    /// Return the package dependency's category, package, version, and revision.
    /// For example, the package dependency "=cat/pkg-1-r2" returns "cat/pkg-1-r2".
    pub fn cpv(&self) -> String {
        if let Some(ver) = &self.version {
            format!("{}-{}", self.cpn, ver.without_op())
        } else {
            format!("{}", self.cpn)
        }
    }

    /// Return a package dependency's slot dependency.
    pub fn slot_dep(&self) -> Option<&SlotDep<S>> {
        self.slot_dep.as_ref()
    }

    /// Return a package dependency's slot.
    pub fn slot(&self) -> Option<&str> {
        self.slot_dep
            .as_ref()
            .and_then(|s| s.slot.as_ref())
            .map(|s| s.slot())
    }

    /// Return a package dependency's subslot.
    pub fn subslot(&self) -> Option<&str> {
        self.slot_dep
            .as_ref()
            .and_then(|s| s.slot.as_ref())
            .and_then(|s| s.subslot())
    }

    /// Return a package dependency's slot operator.
    pub fn slot_op(&self) -> Option<SlotOperator> {
        self.slot_dep.as_ref().and_then(|s| s.op)
    }

    /// Return a package dependency's repository.
    pub fn repo(&self) -> Option<&str> {
        self.repo.as_ref().map(|s| s.as_ref())
    }

    /// Return a key value used to implement various traits, e.g. Eq, Hash, etc.
    fn key(&self) -> DepKey<S> {
        (self.cpn(), self.version(), self.blocker(), self.slot_dep(), self.use_deps(), self.repo())
    }
}

/// Compare two package dependencies.
fn cmp<S1, S2>(d1: &Dep<S1>, d2: &Dep<S2>) -> Ordering
where
    S1: Stringable,
    S2: Stringable,
{
    cmp_not_equal!(d1.category(), d2.category());
    cmp_not_equal!(d1.package(), d2.package());
    partial_cmp_opt_not_equal!(&d1.version, &d2.version);
    cmp_not_equal!(&d1.blocker, &d2.blocker);
    partial_cmp_opt_not_equal!(&d1.slot_dep, &d2.slot_dep);
    partial_cmp_opt_not_equal!(&d1.use_deps, &d2.use_deps);
    cmp_not_equal!(&d1.repo(), &d2.repo());
    Ordering::Equal
}

impl<S: Stringable> fmt::Display for Dep<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // append blocker
        if let Some(blocker) = &self.blocker {
            write!(f, "{blocker}")?;
        }

        // append version operator with cpv
        let cpv = self.cpv();
        use Operator::*;
        match self.op() {
            None => write!(f, "{cpv}")?,
            Some(Less) => write!(f, "<{cpv}")?,
            Some(LessOrEqual) => write!(f, "<={cpv}")?,
            Some(Equal) => write!(f, "={cpv}")?,
            Some(EqualGlob) => write!(f, "={cpv}*")?,
            Some(Approximate) => write!(f, "~{cpv}")?,
            Some(GreaterOrEqual) => write!(f, ">={cpv}")?,
            Some(Greater) => write!(f, ">{cpv}")?,
        }

        // append slot dep
        if let Some(slot_dep) = &self.slot_dep {
            write!(f, ":{slot_dep}")?;
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

/// Determine if a package dependency intersects with a Cpv.
fn dep_intersects_cpv<S1, S2>(dep: &Dep<S1>, cpv: &Cpv<S2>) -> bool
where
    S1: Stringable,
    S2: Stringable,
{
    bool_not_equal!(dep.category(), cpv.category());
    bool_not_equal!(dep.package(), cpv.package());
    dep.version()
        .map(|v| v.intersects(cpv.version()))
        .unwrap_or(true)
}

impl<S1: Stringable, S2: Stringable> Intersects<Cpv<S1>> for Dep<S2> {
    fn intersects(&self, other: &Cpv<S1>) -> bool {
        dep_intersects_cpv(self, other)
    }
}

impl<S1: Stringable, S2: Stringable> Intersects<Dep<S1>> for Cpv<S2> {
    fn intersects(&self, other: &Dep<S1>) -> bool {
        dep_intersects_cpv(other, self)
    }
}

impl<S1: Stringable, S2: Stringable> Intersects<Cpv<S1>> for Cow<'_, Dep<S2>> {
    fn intersects(&self, other: &Cpv<S1>) -> bool {
        dep_intersects_cpv(self, other)
    }
}

impl<S1: Stringable, S2: Stringable> Intersects<Cow<'_, Dep<S1>>> for Cpv<S2> {
    fn intersects(&self, other: &Cow<'_, Dep<S1>>) -> bool {
        dep_intersects_cpv(other, self)
    }
}

/// Determine if two package dependencies intersect ignoring blockers.
fn dep_intersects<S1, S2>(dep1: &Dep<S1>, dep2: &Dep<S2>) -> bool
where
    S1: Stringable,
    S2: Stringable,
{
    bool_not_equal!(&dep1.category(), &dep2.category());
    bool_not_equal!(&dep1.package(), &dep2.package());

    if let (Some(x), Some(y)) = (dep1.slot(), dep2.slot()) {
        bool_not_equal!(x, y);
    }

    if let (Some(x), Some(y)) = (dep1.subslot(), dep2.subslot()) {
        bool_not_equal!(x, y);
    }

    if let (Some(s1), Some(s2)) = (dep1.use_deps(), dep2.use_deps()) {
        // convert USE dep sets to the same type
        let s1: HashSet<_> = s1.iter().map(|x| x.to_ref()).collect();
        let s2: HashSet<_> = s2.iter().map(|x| x.to_ref()).collect();

        // find the differences between the sets
        let mut use_map = HashMap::<_, HashSet<_>>::new();
        for u in s1.symmetric_difference(&s2) {
            use_map.entry(&u.flag).or_default().insert(&u.kind);
        }

        // determine if the set of differences contains a flag both enabled and disabled
        for kinds in use_map.values() {
            if kinds.contains(&UseDepKind::Disabled) && kinds.contains(&UseDepKind::Enabled) {
                return false;
            }
        }
    }

    if let (Some(x), Some(y)) = (dep1.repo(), dep2.repo()) {
        bool_not_equal!(x, y);
    }

    if let (Some(x), Some(y)) = (dep1.version(), dep2.version()) {
        x.intersects(y)
    } else {
        true
    }
}

impl<S1: Stringable, S2: Stringable> Intersects<Dep<S1>> for Dep<S2> {
    fn intersects(&self, other: &Dep<S1>) -> bool {
        dep_intersects(self, other)
    }
}

impl<S1: Stringable, S2: Stringable> Intersects<Cow<'_, Dep<S1>>> for Cow<'_, Dep<S2>> {
    fn intersects(&self, other: &Cow<'_, Dep<S1>>) -> bool {
        dep_intersects(self, other)
    }
}

impl<S1: Stringable, S2: Stringable> Intersects<Dep<S1>> for Cow<'_, Dep<S2>> {
    fn intersects(&self, other: &Dep<S1>) -> bool {
        dep_intersects(self, other)
    }
}

impl<S1: Stringable, S2: Stringable> Intersects<Cow<'_, Dep<S1>>> for Dep<S2> {
    fn intersects(&self, other: &Cow<'_, Dep<S1>>) -> bool {
        dep_intersects(self, other)
    }
}

equivalent!(Dep);

#[cfg(test)]
mod tests {
    use indexmap::IndexSet;

    use crate::eapi::{self, EAPIS};
    use crate::test::TEST_DATA;
    use crate::utils::hash;

    use super::*;

    #[test]
    fn new_and_parse() {
        // invalid
        for s in &TEST_DATA.dep_toml.invalid {
            for eapi in &*EAPIS {
                let result = Dep::parse(s, Some(*eapi));
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
                let result = Dep::parse(s, Some(*eapi));
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
                let result = Dep::parse(s, Some(*eapi));
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
            let dep: Dep<_> = s.parse().unwrap();
            assert_eq!(dep.to_string(), s);
        }
    }

    #[test]
    fn cpn() {
        for (s, cpn) in [
            ("cat/pkg", "cat/pkg"),
            ("<cat/pkg-4", "cat/pkg"),
            ("<=cat/pkg-4-r1", "cat/pkg"),
            ("=cat/pkg-4", "cat/pkg"),
            ("=cat/pkg-4*", "cat/pkg"),
            ("~cat/pkg-4", "cat/pkg"),
            (">=cat/pkg-r1-2-r3", "cat/pkg-r1"),
            (">cat/pkg-4-r1:0=", "cat/pkg"),
        ] {
            let dep: Dep<_> = s.parse().unwrap();
            assert_eq!(dep.cpn(), &Cpn::try_new(cpn).unwrap());
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
            let dep: Dep<_> = s.parse().unwrap();
            let version = version.map(|s| parse::version_with_op(s).into_owned().unwrap());
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
            let dep: Dep<_> = s.parse().unwrap();
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
            let dep: Dep<_> = s.parse().unwrap();
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
            let dep: Dep<_> = s.parse().unwrap();
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
            let d1_owned = Dep::try_new(s1).unwrap();
            let d1_borrowed = Dep::parse(s1, None).unwrap();
            let d1_cow = d1_owned.without([]).unwrap();
            let d2_owned = Dep::try_new(s2).unwrap();
            let d2_borrowed = Dep::parse(s2, None).unwrap();
            let d2_cow = d2_owned.without([]).unwrap();
            if op == "!=" {
                // lhs != rhs
                assert_ne!(d1_owned, d2_owned, "failed: {expr}");
                assert_ne!(d1_borrowed, d2_borrowed, "failed: {expr}");
                assert_ne!(d1_cow, d2_cow, "failed: {expr}");
                assert_ne!(d1_owned, d2_borrowed, "failed: {expr}");
                assert_ne!(d1_owned, d2_cow, "failed: {expr}");
                assert_ne!(d1_borrowed, d2_owned, "failed: {expr}");
                assert_ne!(d1_borrowed, d2_cow, "failed: {expr}");
                assert_ne!(d1_cow, d2_owned, "failed: {expr}");
                assert_ne!(d1_cow, d2_borrowed, "failed: {expr}");

                // rhs != lhs
                assert_ne!(d2_owned, d1_owned, "failed: {expr}");
                assert_ne!(d2_borrowed, d1_borrowed, "failed: {expr}");
                assert_ne!(d2_cow, d1_cow, "failed: {expr}");
                assert_ne!(d2_owned, d1_borrowed, "failed: {expr}");
                assert_ne!(d2_owned, d1_cow, "failed: {expr}");
                assert_ne!(d2_borrowed, d1_owned, "failed: {expr}");
                assert_ne!(d2_borrowed, d1_cow, "failed: {expr}");
                assert_ne!(d2_cow, d1_owned, "failed: {expr}");
                assert_ne!(d2_cow, d1_borrowed, "failed: {expr}");
            } else {
                let op = op_map[op];
                // like types
                assert_eq!(d1_owned.cmp(&d2_owned), op, "failed: {expr}");
                assert_eq!(d2_owned.cmp(&d1_owned), op.reverse(), "failed inverted: {expr}");
                assert_eq!(d1_borrowed.cmp(&d2_borrowed), op, "failed: {expr}");
                assert_eq!(d2_borrowed.cmp(&d1_borrowed), op.reverse(), "failed inverted: {expr}");
                assert_eq!(d1_cow.cmp(&d2_cow), op, "failed: {expr}");
                assert_eq!(d2_cow.cmp(&d1_cow), op.reverse(), "failed inverted: {expr}");

                // different types
                assert_eq!(d1_owned.partial_cmp(&d2_borrowed), Some(op), "failed: {expr}");
                assert_eq!(d1_owned.partial_cmp(&d2_cow), Some(op), "failed: {expr}");
                assert_eq!(d1_borrowed.partial_cmp(&d2_owned), Some(op), "failed: {expr}");
                assert_eq!(d1_borrowed.partial_cmp(&d2_cow), Some(op), "failed: {expr}");
                assert_eq!(d1_cow.partial_cmp(&d2_owned), Some(op), "failed: {expr}");
                assert_eq!(d1_cow.partial_cmp(&d2_borrowed), Some(op), "failed: {expr}");
                assert_eq!(
                    d2_owned.partial_cmp(&d1_borrowed),
                    Some(op.reverse()),
                    "failed inverted: {expr}"
                );
                assert_eq!(
                    d2_owned.partial_cmp(&d1_cow),
                    Some(op.reverse()),
                    "failed inverted: {expr}"
                );
                assert_eq!(
                    d2_borrowed.partial_cmp(&d1_owned),
                    Some(op.reverse()),
                    "failed inverted: {expr}"
                );
                assert_eq!(
                    d2_borrowed.partial_cmp(&d1_cow),
                    Some(op.reverse()),
                    "failed inverted: {expr}"
                );
                assert_eq!(
                    d2_cow.partial_cmp(&d1_owned),
                    Some(op.reverse()),
                    "failed inverted: {expr}"
                );
                assert_eq!(
                    d2_cow.partial_cmp(&d1_borrowed),
                    Some(op.reverse()),
                    "failed inverted: {expr}"
                );

                // verify the following property holds since both Hash and Eq are implemented:
                // k1 == k2 -> hash(k1) == hash(k2)
                if op == Ordering::Equal {
                    assert_eq!(hash(&d1_owned), hash(&d2_owned), "failed hash: {expr}");
                    assert_eq!(hash(&d1_borrowed), hash(&d2_borrowed), "failed hash: {expr}");
                    assert_eq!(hash(&d1_cow), hash(&d2_cow), "failed hash: {expr}");
                    assert_eq!(hash(&d1_owned), hash(&d2_borrowed), "failed hash: {expr}");
                    assert_eq!(hash(&d1_owned), hash(&d2_cow), "failed hash: {expr}");
                    assert_eq!(hash(&d1_borrowed), hash(&d2_owned), "failed hash: {expr}");
                    assert_eq!(hash(&d1_borrowed), hash(&d2_cow), "failed hash: {expr}");
                    assert_eq!(hash(&d1_cow), hash(&d2_owned), "failed hash: {expr}");
                    assert_eq!(hash(&d1_cow), hash(&d2_borrowed), "failed hash: {expr}");
                }
            }
        }
    }

    #[test]
    fn intersects() {
        // inject version intersects data from version.toml into Dep objects
        let dep = Dep::try_new("a/b").unwrap();
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
                let obj1_owned = Dep::try_new(s1).unwrap();
                let obj1_borrowed = Dep::parse(s1, None).unwrap();
                let obj1_cow = obj1_owned.without([]).unwrap();
                let obj2_owned = Dep::try_new(s2).unwrap();
                let obj2_borrowed = Dep::parse(s2, None).unwrap();
                let obj2_cow = obj2_owned.without([]).unwrap();

                // self intersection
                assert!(obj1_owned.intersects(&obj1_owned), "{s1} doesn't intersect {s1}");
                assert!(obj1_borrowed.intersects(&obj1_borrowed), "{s1} doesn't intersect {s1}");
                assert!(obj1_cow.intersects(&obj1_cow), "{s1} doesn't intersect {s1}");
                assert!(obj1_owned.intersects(&obj1_borrowed), "{s1} doesn't intersect {s1}");
                assert!(obj1_owned.intersects(&obj1_cow), "{s1} doesn't intersect {s1}");
                assert!(obj1_borrowed.intersects(&obj1_owned), "{s1} doesn't intersect {s1}");
                assert!(obj1_borrowed.intersects(&obj1_cow), "{s1} doesn't intersect {s1}");
                assert!(obj1_cow.intersects(&obj1_owned), "{s1} doesn't intersect {s1}");
                assert!(obj1_cow.intersects(&obj1_borrowed), "{s1} doesn't intersect {s1}");
                assert!(obj2_owned.intersects(&obj2_owned), "{s2} doesn't intersect {s2}");
                assert!(obj2_borrowed.intersects(&obj2_borrowed), "{s2} doesn't intersect {s2}");
                assert!(obj2_cow.intersects(&obj2_cow), "{s2} doesn't intersect {s2}");
                assert!(obj2_owned.intersects(&obj2_borrowed), "{s2} doesn't intersect {s2}");
                assert!(obj2_owned.intersects(&obj2_cow), "{s2} doesn't intersect {s2}");
                assert!(obj2_borrowed.intersects(&obj2_owned), "{s2} doesn't intersect {s2}");
                assert!(obj2_borrowed.intersects(&obj2_cow), "{s2} doesn't intersect {s2}");
                assert!(obj2_cow.intersects(&obj2_owned), "{s2} doesn't intersect {s2}");
                assert!(obj2_cow.intersects(&obj2_borrowed), "{s2} doesn't intersect {s2}");

                // intersects depending on status
                if d.status {
                    assert!(obj1_owned.intersects(&obj2_owned), "{s1} doesn't intersect {s2}");
                    assert!(
                        obj1_borrowed.intersects(&obj2_borrowed),
                        "{s1} doesn't intersect {s2}"
                    );
                    assert!(obj1_cow.intersects(&obj2_cow), "{s1} doesn't intersect {s2}");
                    assert!(obj1_owned.intersects(&obj2_borrowed), "{s1} doesn't intersect {s2}");
                    assert!(obj1_owned.intersects(&obj2_cow), "{s1} doesn't intersect {s2}");
                    assert!(obj1_borrowed.intersects(&obj2_owned), "{s1} doesn't intersect {s2}");
                    assert!(obj1_borrowed.intersects(&obj2_cow), "{s1} doesn't intersect {s2}");
                    assert!(obj1_cow.intersects(&obj2_owned), "{s1} doesn't intersect {s2}");
                    assert!(obj1_cow.intersects(&obj2_borrowed), "{s1} doesn't intersect {s2}");
                } else {
                    assert!(!obj1_owned.intersects(&obj2_owned), "{s1} intersects {s2}");
                    assert!(!obj1_borrowed.intersects(&obj2_borrowed), "{s1} intersects {s2}");
                    assert!(!obj1_cow.intersects(&obj2_cow), "{s1} intersects {s2}");
                    assert!(!obj1_owned.intersects(&obj2_borrowed), "{s1} intersects {s2}");
                    assert!(!obj1_owned.intersects(&obj2_cow), "{s1} intersects {s2}");
                    assert!(!obj1_borrowed.intersects(&obj2_owned), "{s1} intersects {s2}");
                    assert!(!obj1_borrowed.intersects(&obj2_cow), "{s1} intersects {s2}");
                    assert!(!obj1_cow.intersects(&obj2_owned), "{s1} intersects {s2}");
                    assert!(!obj1_cow.intersects(&obj2_borrowed), "{s1} intersects {s2}");
                }
            }
        }
    }

    #[test]
    fn versioned() {
        for (s, expected) in [
            ("!!>=cat/pkg-1.2-r3:4/5=::repo[a,b]", "=cat/pkg-1.2-r3"),
            ("=cat/pkg-1", "=cat/pkg-1"),
            ("cat/pkg", "cat/pkg"),
        ] {
            let dep = Dep::try_new(s).unwrap();
            let expected = Dep::try_new(expected).unwrap();
            assert_eq!(dep.versioned(), expected);
        }
    }

    #[test]
    fn unversioned() {
        let expected = Dep::try_new("cat/pkg").unwrap();
        for s in ["!!>=cat/pkg-1.2-r3:4/5=::repo[a,b]", "=cat/pkg-1", "cat/pkg"] {
            let dep = Dep::try_new(s).unwrap();
            assert_eq!(dep.unversioned(), expected);
        }
    }

    #[test]
    fn no_use_deps() {
        for (s, expected) in [
            ("!!>=cat/pkg-1.2-r3:4/5=::repo[a,b]", "!!>=cat/pkg-1.2-r3:4/5=::repo"),
            ("=cat/pkg-1[a,b,c]", "=cat/pkg-1"),
            ("cat/pkg", "cat/pkg"),
        ] {
            let dep = Dep::try_new(s).unwrap();
            let expected = Dep::try_new(expected).unwrap();
            assert_eq!(dep.no_use_deps(), expected);
        }
    }

    #[test]
    fn without() {
        let dep = Dep::try_new("!!>=cat/pkg-1.2-r3:4/5=::repo[a,b]").unwrap();

        for (field, expected) in [
            (DepField::Blocker, ">=cat/pkg-1.2-r3:4/5=::repo[a,b]"),
            (DepField::SlotDep, "!!>=cat/pkg-1.2-r3::repo[a,b]"),
            (DepField::Version, "!!cat/pkg:4/5=::repo[a,b]"),
            (DepField::UseDeps, "!!>=cat/pkg-1.2-r3:4/5=::repo"),
            (DepField::Repo, "!!>=cat/pkg-1.2-r3:4/5=[a,b]"),
        ] {
            let d = dep.without([field]).unwrap();
            let s = d.to_string();
            assert_eq!(&s, expected);
            assert_eq!(d.as_ref(), &Dep::try_new(&s).unwrap());
        }

        // remove all fields
        let d = dep.without(DepField::optional()).unwrap();
        let s = d.to_string();
        assert_eq!(&s, "cat/pkg");
        assert_eq!(d.as_ref(), &Dep::try_new(&s).unwrap());

        // verify all combinations of dep fields create valid deps
        for vals in DepField::optional().powerset() {
            let d = dep.without(vals).unwrap();
            let s = d.to_string();
            assert_eq!(d.as_ref(), &Dep::try_new(&s).unwrap());
        }

        // no changes returns a borrowed dep
        let dep = Dep::try_new("cat/pkg").unwrap();
        matches!(dep.without([DepField::Version]).unwrap(), Cow::Borrowed(_));
    }

    #[test]
    fn modify() {
        let dep = Dep::try_new("!!>=cat/pkg-1.2-r3:4::repo[a,b]").unwrap();

        // modify single fields
        for (field, val, expected) in [
            (DepField::Category, "a", "!!>=a/pkg-1.2-r3:4::repo[a,b]"),
            (DepField::Package, "b", "!!>=cat/b-1.2-r3:4::repo[a,b]"),
            (DepField::Blocker, "!", "!>=cat/pkg-1.2-r3:4::repo[a,b]"),
            (DepField::SlotDep, "1", "!!>=cat/pkg-1.2-r3:1::repo[a,b]"),
            (DepField::SlotDep, "1/2", "!!>=cat/pkg-1.2-r3:1/2::repo[a,b]"),
            (DepField::SlotDep, "1/2=", "!!>=cat/pkg-1.2-r3:1/2=::repo[a,b]"),
            (DepField::SlotDep, "*", "!!>=cat/pkg-1.2-r3:*::repo[a,b]"),
            (DepField::SlotDep, "=", "!!>=cat/pkg-1.2-r3:=::repo[a,b]"),
            (DepField::Version, "<0", "!!<cat/pkg-0:4::repo[a,b]"),
            (DepField::UseDeps, "x,y,z", "!!>=cat/pkg-1.2-r3:4::repo[x,y,z]"),
            (DepField::Repo, "test", "!!>=cat/pkg-1.2-r3:4::test[a,b]"),
        ] {
            let d = dep.modify([(field, Some(val))]).unwrap();
            let s = d.to_string();
            assert_eq!(&s, expected);
            assert_eq!(d.as_ref(), &Dep::try_new(&s).unwrap());
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
            (DepField::SlotDep, Some("1/2=")),
            (DepField::Version, Some("<0")),
            (DepField::UseDeps, Some("x,y,z")),
            (DepField::Repo, Some("test")),
        ];
        for vals in fields.into_iter().powerset() {
            let d = dep.modify(vals).unwrap();
            let s = d.to_string();
            assert_eq!(d.as_ref(), &Dep::try_new(&s).unwrap());
        }

        // verify all combinations of removing optional dep fields create valid deps
        for vals in DepField::optional().powerset() {
            let d = dep.modify(vals.into_iter().map(|f| (f, None))).unwrap();
            let s = d.to_string();
            assert_eq!(d.as_ref(), &Dep::try_new(&s).unwrap());
        }

        // invalid values
        assert!(dep.modify([(DepField::Category, Some("-cat"))]).is_err());
        assert!(dep.modify([(DepField::Package, Some("pkg-1a-1"))]).is_err());
        assert!(dep.modify([(DepField::Blocker, Some("!!!"))]).is_err());
        assert!(dep.modify([(DepField::SlotDep, Some(":1"))]).is_err());
        assert!(dep.modify([(DepField::Version, Some("1"))]).is_err());
        assert!(dep.modify([(DepField::UseDeps, Some("+u1,u2"))]).is_err());
        assert!(dep.modify([(DepField::Repo, Some("pkg-1a-1"))]).is_err());

        // no changes returns a borrowed dep
        let dep = Dep::try_new("cat/pkg").unwrap();
        let d = dep.modify([(DepField::Category, Some("cat"))]).unwrap();
        assert!(matches!(d, Cow::Borrowed(_)));
        let d = dep.modify([(DepField::Version, None)]).unwrap();
        assert!(matches!(d, Cow::Borrowed(_)));
    }

    #[test]
    fn sorting() {
        for d in &TEST_DATA.dep_toml.sorting {
            let mut reversed: Vec<Dep<_>> =
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
            let set: HashSet<Dep<_>> = d
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
