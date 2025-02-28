use std::fmt;

use strum::IntoEnumIterator;

use crate::repo::ebuild::Eclass;

use super::phase::PhaseKind;

/// Marker used to denote valid or current build state scope.
#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum Scope {
    #[default]
    Global,
    Eclass(Option<Eclass>),
    Phase(PhaseKind),
}

impl Scope {
    /// Determine if the scope is an eclass scope.
    pub(crate) fn is_eclass(&self) -> bool {
        matches!(self, Self::Eclass(_))
    }
}

impl From<Eclass> for Scope {
    fn from(value: Eclass) -> Self {
        Self::Eclass(Some(value))
    }
}

impl From<PhaseKind> for Scope {
    fn from(value: PhaseKind) -> Self {
        Self::Phase(value)
    }
}

impl AsRef<str> for Scope {
    fn as_ref(&self) -> &str {
        match self {
            Self::Eclass(_) => "eclass",
            Self::Global => "global",
            Self::Phase(p) => p.as_ref(),
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

/// Multi-scope type for EAPI registration.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum EbuildScope {
    All,
    Eclass,
    Global,
    Phases,
    Pkg,
    Src,
    Phase(PhaseKind),
}

impl From<PhaseKind> for EbuildScope {
    fn from(value: PhaseKind) -> Self {
        Self::Phase(value)
    }
}

impl EbuildScope {
    pub(crate) fn iter(&self) -> impl Iterator<Item = Scope> {
        self.into_iter()
    }
}

impl PartialEq<Scope> for EbuildScope {
    fn eq(&self, other: &Scope) -> bool {
        match (self, other) {
            (Self::All, _) => true,
            (Self::Eclass, Scope::Eclass(_)) => true,
            (Self::Global, Scope::Global) => true,
            (Self::Phases, Scope::Phase(_)) => true,
            (Self::Pkg, Scope::Phase(x)) => x.as_ref().starts_with("pkg_"),
            (Self::Src, Scope::Phase(x)) => x.as_ref().starts_with("src_"),
            (Self::Phase(x), Scope::Phase(y)) => x == y,
            _ => false,
        }
    }
}

impl PartialEq<PhaseKind> for EbuildScope {
    fn eq(&self, other: &PhaseKind) -> bool {
        let scope = Scope::Phase(*other);
        *self == scope
    }
}

impl IntoIterator for EbuildScope {
    type Item = Scope;
    type IntoIter = Box<dyn Iterator<Item = Scope>>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::All => Box::new([Self::Global, Self::Eclass, Self::Phases].iter().flatten()),
            Self::Eclass => Box::new([Scope::Eclass(None)].into_iter()),
            Self::Global => Box::new([Scope::Global].into_iter()),
            Self::Phases => Box::new(PhaseKind::iter().map(Scope::Phase)),
            Self::Pkg => Box::new(
                Self::Phases
                    .iter()
                    .filter(|k| k.as_ref().starts_with("pkg_")),
            ),
            Self::Src => Box::new(
                Self::Phases
                    .iter()
                    .filter(|k| k.as_ref().starts_with("src_")),
            ),
            Self::Phase(p) => Box::new([Scope::Phase(p)].into_iter()),
        }
    }
}

impl IntoIterator for &EbuildScope {
    type Item = Scope;
    type IntoIter = Box<dyn Iterator<Item = Scope>>;

    fn into_iter(self) -> Self::IntoIter {
        (*self).into_iter()
    }
}
