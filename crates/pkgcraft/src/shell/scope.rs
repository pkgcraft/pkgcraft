use std::borrow::Borrow;
use std::fmt;

use strum::IntoEnumIterator;

use crate::repo::ebuild::Eclass;

use super::phase::{Phase, PhaseKind};

#[derive(Debug, Default, PartialEq, Eq, Hash, Ord, PartialOrd, Copy, Clone)]
pub(crate) enum Scope {
    #[default]
    Global,
    Eclass(Option<&'static Eclass>),
    Phase(PhaseKind),
}

impl Scope {
    pub(crate) fn is_eclass(&self) -> bool {
        matches!(self, Self::Eclass(_))
    }
}

impl<T: Borrow<Phase>> From<T> for Scope {
    fn from(phase: T) -> Self {
        Scope::Phase(phase.borrow().into())
    }
}

impl AsRef<str> for Scope {
    fn as_ref(&self) -> &str {
        match self {
            Self::Eclass(Some(eclass)) => eclass.as_ref(),
            Self::Eclass(None) => "eclass",
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

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
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

impl IntoIterator for &Scopes {
    type Item = Scope;
    type IntoIter = Box<dyn Iterator<Item = Scope>>;

    fn into_iter(self) -> Self::IntoIter {
        use Scopes::*;
        match self {
            Eclass => Box::new([Scope::Eclass(None)].into_iter()),
            Global => Box::new([Scope::Global].into_iter()),
            Phases => Box::new(PhaseKind::iter().map(Scope::Phase)),
            Src => Box::new(Phases.iter().filter(|k| k.as_ref().starts_with("src_"))),
            Pkg => Box::new(Phases.iter().filter(|k| k.as_ref().starts_with("pkg_"))),
            All => Box::new([Global, Eclass, Phases].iter().flatten()),
            Phase(p) => Box::new([Scope::Phase(*p)].into_iter()),
        }
    }
}

impl IntoIterator for Scopes {
    type Item = Scope;
    type IntoIter = Box<dyn Iterator<Item = Scope>>;

    fn into_iter(self) -> Self::IntoIter {
        (&self).into_iter()
    }
}
