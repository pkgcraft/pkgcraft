use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use itertools::Itertools;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{AsRefStr, Display, EnumString};

use crate::macros::bool_not_equal;
use crate::traits::Intersects;
use crate::types::{OrderedMap, OrderedSet, SortedSet};
use crate::Error;

use super::use_dep::{UseDep, UseDepKind};
use super::version::{Operator, Revision, Version};
use super::{parse, Cpn, Cpv};

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
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Slot {
    pub(crate) name: String,
}

impl Default for Slot {
    fn default() -> Self {
        Self { name: "0".to_string() }
    }
}

impl Slot {
    /// Create a new Slot from a given string.
    pub fn try_new(s: &str) -> crate::Result<Self> {
        parse::slot(s)
    }

    /// Return the main slot value.
    pub fn slot(&self) -> &str {
        self.name.split_once('/').map_or(&self.name, |x| x.0)
    }

    /// Return the subslot value if it exists.
    pub fn subslot(&self) -> Option<&str> {
        self.name.split_once('/').map(|x| x.1)
    }
}

impl PartialEq<str> for Slot {
    fn eq(&self, other: &str) -> bool {
        self.name == other
    }
}

impl PartialEq<Slot> for &str {
    fn eq(&self, other: &Slot) -> bool {
        other == *self
    }
}

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// Package slot dependency.
#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Hash, Clone)]
pub enum SlotDep {
    Op(SlotOperator),
    Slot(Slot),
    SlotOp(Slot, SlotOperator),
}

impl fmt::Display for SlotDep {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Op(op) => write!(f, "{op}"),
            Self::Slot(slot) => write!(f, "{slot}"),
            Self::SlotOp(slot, op) => write!(f, "{slot}{op}"),
        }
    }
}

impl SlotDep {
    /// Create a new SlotDep from a given string.
    pub fn try_new(s: &str) -> crate::Result<Self> {
        parse::slot_dep(s)
    }

    /// Return the slot if it exists.
    pub fn slot(&self) -> Option<&Slot> {
        match self {
            Self::Op(_) => None,
            Self::Slot(slot) => Some(slot),
            Self::SlotOp(slot, _) => Some(slot),
        }
    }

    /// Return the slot operator if it exists.
    pub fn op(&self) -> Option<SlotOperator> {
        match self {
            Self::Op(op) => Some(*op),
            Self::Slot(_) => None,
            Self::SlotOp(_, op) => Some(*op),
        }
    }
}

impl FromStr for SlotDep {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::try_new(s)
    }
}

/// Package dependency.
#[derive(SerializeDisplay, DeserializeFromStr, Clone)]
pub struct Dep {
    pub(crate) cpn: Cpn,
    pub(crate) blocker: Option<Blocker>,
    pub(crate) version: Option<Version>,
    pub(crate) slot_dep: Option<SlotDep>,
    pub(crate) use_deps: Option<SortedSet<UseDep>>,
    pub(crate) repo: Option<String>,
}

impl From<Cpn> for Dep {
    fn from(cpn: Cpn) -> Self {
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

impl PartialEq for Dep {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl PartialEq<Cow<'_, Dep>> for Dep {
    fn eq(&self, other: &Cow<'_, Dep>) -> bool {
        self == other.as_ref()
    }
}

impl PartialEq<Dep> for Cow<'_, Dep> {
    fn eq(&self, other: &Dep) -> bool {
        other == self
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

impl PartialOrd<Cow<'_, Dep>> for Dep {
    fn partial_cmp(&self, other: &Cow<'_, Dep>) -> Option<Ordering> {
        Some(self.cmp(other.as_ref()))
    }
}

impl PartialOrd<Dep> for Cow<'_, Dep> {
    fn partial_cmp(&self, other: &Dep) -> Option<Ordering> {
        Some(self.as_ref().cmp(other))
    }
}

impl FromStr for Dep {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::try_new(s)
    }
}

/// Key type used for implementing various traits, e.g. Eq, Hash, etc.
type DepKey<'a> = (
    &'a Cpn,                       // unversioned dep
    Option<&'a Version>,           // version
    Option<Blocker>,               // blocker
    Option<&'a SlotDep>,           // slot
    Option<&'a SortedSet<UseDep>>, // use deps
    Option<&'a str>,               // repo
);

impl Dep {
    /// Used by the parser to inject attributes.
    pub(crate) fn with(
        mut self,
        blocker: Option<Blocker>,
        slot_dep: Option<SlotDep>,
        use_deps: Option<Vec<UseDep>>,
        repo: Option<&str>,
    ) -> Self {
        self.blocker = blocker;
        self.slot_dep = slot_dep;
        self.use_deps = use_deps.map(|u| u.into_iter().collect());
        self.repo = repo.map(|x| x.to_string());
        self
    }

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
                        let val = s
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
                DepField::SlotDep => {
                    if let Some(s) = s {
                        let val = s.parse()?;
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

impl Dep {
    /// Return a package dependency's [`Cpn`].
    pub fn cpn(&self) -> &Cpn {
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

    /// Return a package dependency's blocker if it exists.
    pub fn blocker(&self) -> Option<Blocker> {
        self.blocker
    }

    /// Return a package dependency's USE flag dependencies if it exists.
    pub fn use_deps(&self) -> Option<&SortedSet<UseDep>> {
        self.use_deps.as_ref()
    }

    /// Return a package dependency's version if it exists.
    pub fn version(&self) -> Option<&Version> {
        self.version.as_ref()
    }

    /// Return a package dependency's revision if it exists.
    pub fn revision(&self) -> Option<&Revision> {
        self.version().and_then(|v| v.revision())
    }

    /// Return a package dependency's version operator if it exists.
    pub fn op(&self) -> Option<Operator> {
        self.version().and_then(|v| v.op())
    }

    /// Return the [`Cpv`] of the package dependency if one exists.
    /// For example, the package dependency "=cat/pkg-1-r2" returns "cat/pkg-1-r2".
    pub fn cpv(&self) -> Option<Cpv> {
        self.version.clone().map(|mut version| {
            version.op = None;
            Cpv { cpn: self.cpn.clone(), version }
        })
    }

    /// Return a package dependency's slot dependency if it exists.
    pub fn slot_dep(&self) -> Option<&SlotDep> {
        self.slot_dep.as_ref()
    }

    /// Return a package dependency's slot if it exists.
    pub fn slot(&self) -> Option<&str> {
        self.slot_dep
            .as_ref()
            .and_then(|s| s.slot())
            .map(|s| s.slot())
    }

    /// Return a package dependency's subslot if it exists.
    pub fn subslot(&self) -> Option<&str> {
        self.slot_dep
            .as_ref()
            .and_then(|s| s.slot())
            .and_then(|s| s.subslot())
    }

    /// Return a package dependency's slot operator if it exists.
    pub fn slot_op(&self) -> Option<SlotOperator> {
        self.slot_dep.as_ref().and_then(|s| s.op())
    }

    /// Return a package dependency's repository if it exists.
    pub fn repo(&self) -> Option<&str> {
        self.repo.as_ref().map(|s| s.as_ref())
    }

    /// Return a key value used to implement various traits, e.g. Eq, Ord, and Hash.
    fn key(&self) -> DepKey {
        (self.cpn(), self.version(), self.blocker(), self.slot_dep(), self.use_deps(), self.repo())
    }
}

impl fmt::Display for Dep {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // append blocker
        if let Some(blocker) = &self.blocker {
            write!(f, "{blocker}")?;
        }

        // append version operator with cpv
        let cpn = &self.cpn;
        if let Some(version) = self.version() {
            let ver = version.without_op();
            match version.op.expect("invalid version") {
                Operator::Less => write!(f, "<{cpn}-{ver}")?,
                Operator::LessOrEqual => write!(f, "<={cpn}-{ver}")?,
                Operator::Equal => write!(f, "={cpn}-{ver}")?,
                Operator::EqualGlob => write!(f, "={cpn}-{ver}*")?,
                Operator::Approximate => write!(f, "~{cpn}-{ver}")?,
                Operator::GreaterOrEqual => write!(f, ">={cpn}-{ver}")?,
                Operator::Greater => write!(f, ">{cpn}-{ver}")?,
            }
        } else {
            write!(f, "{cpn}")?;
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

impl fmt::Debug for Dep {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Dep {{ {self} }}")
    }
}

impl Intersects<Dep> for Cpv {
    fn intersects(&self, other: &Dep) -> bool {
        bool_not_equal!(self.cpn(), other.cpn());
        other
            .version()
            .map(|v| v.intersects(self.version()))
            .unwrap_or(true)
    }
}

impl Intersects<Cpv> for Dep {
    fn intersects(&self, other: &Cpv) -> bool {
        other.intersects(self)
    }
}

impl Intersects<Cow<'_, Dep>> for Cpv {
    fn intersects(&self, other: &Cow<'_, Dep>) -> bool {
        self.intersects(other.as_ref())
    }
}

impl Intersects<Cpv> for Cow<'_, Dep> {
    fn intersects(&self, other: &Cpv) -> bool {
        other.intersects(self.as_ref())
    }
}

// Determine if any use dependency differences with matching flags are
// requested to be both enabled and disabled.
impl Intersects for SortedSet<UseDep> {
    fn intersects(&self, other: &Self) -> bool {
        !self
            .symmetric_difference(other)
            .filter(|x| x.kind() == UseDepKind::Enabled)
            .map(|x| (x.flag(), x.enabled()))
            .collect::<OrderedMap<_, OrderedSet<_>>>()
            .into_values()
            .any(|vals| vals.len() == 2)
    }
}

impl Intersects for Dep {
    fn intersects(&self, other: &Self) -> bool {
        bool_not_equal!(self.cpn(), other.cpn());

        if let (Some(x), Some(y)) = (self.slot(), other.slot()) {
            bool_not_equal!(x, y);
        }

        if let (Some(x), Some(y)) = (self.subslot(), other.subslot()) {
            bool_not_equal!(x, y);
        }

        if let (Some(s1), Some(s2)) = (self.use_deps(), other.use_deps()) {
            bool_not_equal!(s1.intersects(s2));
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

impl Intersects<Cow<'_, Dep>> for Cow<'_, Dep> {
    fn intersects(&self, other: &Cow<'_, Dep>) -> bool {
        self.as_ref().intersects(other.as_ref())
    }
}

impl Intersects<Cow<'_, Dep>> for Dep {
    fn intersects(&self, other: &Cow<'_, Dep>) -> bool {
        self.intersects(other.as_ref())
    }
}

impl Intersects<Dep> for Cow<'_, Dep> {
    fn intersects(&self, other: &Dep) -> bool {
        other.intersects(self.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use crate::eapi::{self, EAPIS};
    use crate::test::TEST_DATA;
    use crate::utils::hash;

    use super::*;

    #[test]
    fn new_and_parse() {
        // invalid
        for s in &TEST_DATA.dep_toml.invalid {
            for eapi in &*EAPIS {
                let result = Dep::try_new(s);
                assert!(result.is_err(), "{s:?} is valid for EAPI={eapi}");
                let result = eapi.dep(s);
                assert!(result.is_err(), "{s:?} didn't fail for EAPI={eapi}");
            }
        }

        // valid
        for e in &TEST_DATA.dep_toml.valid {
            let s = e.dep.as_str();
            let passing_eapis: OrderedSet<_> = eapi::range(&e.eapis).unwrap().collect();
            for eapi in &passing_eapis {
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
                let result = eapi.dep(s);
                assert!(result.is_err(), "{s:?} didn't fail for EAPI={eapi}");
            }
        }
    }

    #[test]
    fn display_and_debug() {
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
            assert!(format!("{dep:?}").contains(s));
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
            let dep: Dep = s.parse().unwrap();
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
            ("cat/pkg", None),
            ("<cat/pkg-4", Some("cat/pkg-4")),
            ("<=cat/pkg-4-r1", Some("cat/pkg-4-r1")),
            ("=cat/pkg-4", Some("cat/pkg-4")),
            ("=cat/pkg-4*", Some("cat/pkg-4")),
            ("~cat/pkg-4", Some("cat/pkg-4")),
            (">=cat/pkg-r1-2-r3", Some("cat/pkg-r1-2-r3")),
            (">cat/pkg-4-r1:0=", Some("cat/pkg-4-r1")),
        ] {
            let dep: Dep = s.parse().unwrap();
            assert_eq!(dep.cpv(), cpv.map(|s| Cpv::try_new(s).unwrap()));
        }
    }

    #[test]
    fn cmp() {
        let op_map: OrderedMap<_, _> =
            [("<", Ordering::Less), ("==", Ordering::Equal), (">", Ordering::Greater)]
                .into_iter()
                .collect();

        for (expr, (s1, op, s2)) in TEST_DATA.dep_toml.compares() {
            let d1 = Dep::try_new(s1).unwrap();
            let d1_cow = d1.without([]).unwrap();
            let d2 = Dep::try_new(s2).unwrap();
            let d2_cow = d2.without([]).unwrap();
            if op == "!=" {
                // lhs != rhs
                assert_ne!(d1, d2, "failed: {expr}");
                assert_ne!(d1_cow, d2_cow, "failed: {expr}");
                assert_ne!(d1, d2_cow, "failed: {expr}");
                assert_ne!(d1_cow, d2, "failed: {expr}");

                // rhs != lhs
                assert_ne!(d2, d1, "failed: {expr}");
                assert_ne!(d2_cow, d1_cow, "failed: {expr}");
                assert_ne!(d2, d1_cow, "failed: {expr}");
                assert_ne!(d2_cow, d1, "failed: {expr}");
            } else {
                let op = op_map[op];
                // like types
                assert_eq!(d1.cmp(&d2), op, "failed: {expr}");
                assert_eq!(d2.cmp(&d1), op.reverse(), "failed inverted: {expr}");
                assert_eq!(d1_cow.cmp(&d2_cow), op, "failed: {expr}");
                assert_eq!(d2_cow.cmp(&d1_cow), op.reverse(), "failed inverted: {expr}");

                // different types
                assert_eq!(d1.partial_cmp(&d2_cow), Some(op), "failed: {expr}");
                assert_eq!(d1_cow.partial_cmp(&d2), Some(op), "failed: {expr}");
                assert_eq!(d2.partial_cmp(&d1_cow), Some(op.reverse()), "failed inverted: {expr}");
                assert_eq!(d2_cow.partial_cmp(&d1), Some(op.reverse()), "failed inverted: {expr}");

                // verify the following property holds since both Hash and Eq are implemented:
                // k1 == k2 -> hash(k1) == hash(k2)
                if op == Ordering::Equal {
                    assert_eq!(hash(&d1), hash(&d2), "failed hash: {expr}");
                    assert_eq!(hash(&d1_cow), hash(&d2_cow), "failed hash: {expr}");
                    assert_eq!(hash(&d1), hash(&d2_cow), "failed hash: {expr}");
                    assert_eq!(hash(&d1_cow), hash(&d2), "failed hash: {expr}");
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
                let d1 = Dep::try_new(s1).unwrap();
                let d1_cow = d1.without([]).unwrap();
                let d2 = Dep::try_new(s2).unwrap();
                let d2_cow = d2.without([]).unwrap();

                // self intersection
                assert!(d1.intersects(&d1), "{s1} doesn't intersect {s1}");
                assert!(d1_cow.intersects(&d1_cow), "{s1} doesn't intersect {s1}");
                assert!(d1.intersects(&d1_cow), "{s1} doesn't intersect {s1}");
                assert!(d1_cow.intersects(&d1), "{s1} doesn't intersect {s1}");
                assert!(d2.intersects(&d2), "{s2} doesn't intersect {s2}");
                assert!(d2_cow.intersects(&d2_cow), "{s2} doesn't intersect {s2}");
                assert!(d2.intersects(&d2_cow), "{s2} doesn't intersect {s2}");
                assert!(d2_cow.intersects(&d2), "{s2} doesn't intersect {s2}");

                // intersects depending on status
                if d.status {
                    assert!(d1.intersects(&d2), "{s1} doesn't intersect {s2}");
                    assert!(d1_cow.intersects(&d2_cow), "{s1} doesn't intersect {s2}");
                    assert!(d1_cow.intersects(&d2_cow), "{s1} doesn't intersect {s2}");
                    assert!(d1_cow.intersects(&d2), "{s1} doesn't intersect {s2}");
                } else {
                    assert!(!d1.intersects(&d2), "{s1} intersects {s2}");
                    assert!(!d1_cow.intersects(&d2_cow), "{s1} intersects {s2}");
                    assert!(!d1.intersects(&d2), "{s1} intersects {s2}");
                    assert!(!d1_cow.intersects(&d2), "{s1} intersects {s2}");
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
            let set: OrderedSet<Dep> = d
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
