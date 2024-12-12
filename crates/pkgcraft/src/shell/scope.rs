use std::fmt;

use strum::IntoEnumIterator;

use crate::repo::ebuild::Eclass;

use super::phase::PhaseKind;

/// Marker used to denote valid or current build state scope.
#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub(crate) enum Scope {
    #[default]
    Global,
    Eclass(Option<Eclass>),
    Phase(PhaseKind),
}

impl Scope {
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
pub(crate) enum Scopes {
    All,
    Eclass,
    Global,
    Phases,
    Src,
    Pkg,
    Phase(PhaseKind),
}

impl From<PhaseKind> for Scopes {
    fn from(value: PhaseKind) -> Self {
        Self::Phase(value)
    }
}

impl Scopes {
    pub(crate) fn iter(&self) -> impl Iterator<Item = Scope> {
        self.into_iter()
    }
}

impl IntoIterator for Scopes {
    type Item = Scope;
    type IntoIter = Box<dyn Iterator<Item = Scope>>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::Eclass => Box::new([Scope::Eclass(None)].into_iter()),
            Self::Global => Box::new([Scope::Global].into_iter()),
            Self::Phases => Box::new(PhaseKind::iter().map(Scope::Phase)),
            Self::Src => Box::new(
                Self::Phases
                    .iter()
                    .filter(|k| k.as_ref().starts_with("src_")),
            ),
            Self::Pkg => Box::new(
                Self::Phases
                    .iter()
                    .filter(|k| k.as_ref().starts_with("pkg_")),
            ),
            Self::All => Box::new([Self::Global, Self::Eclass, Self::Phases].iter().flatten()),
            Self::Phase(p) => Box::new([Scope::Phase(p)].into_iter()),
        }
    }
}

impl IntoIterator for &Scopes {
    type Item = Scope;
    type IntoIter = Box<dyn Iterator<Item = Scope>>;

    fn into_iter(self) -> Self::IntoIter {
        (*self).into_iter()
    }
}
