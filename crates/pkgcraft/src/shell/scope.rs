use std::fmt;

use strum::IntoEnumIterator;

use super::phase::PhaseKind;

/// Build state scope.
#[derive(Debug, Default, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Scope {
    #[default]
    Global,
    Eclass,
    Phase(PhaseKind),
}

impl Scope {
    /// Determine if the scope is global scope.
    pub(crate) fn is_global(&self) -> bool {
        self == &Self::Global
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
            Self::Eclass => "eclass",
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

/// Scope sets used to represent groups of related scopes.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum ScopeSet {
    All,
    Eclass,
    Global,
    Phases,
    Pkg,
    Src,
    Phase(PhaseKind),
}

impl From<PhaseKind> for ScopeSet {
    fn from(value: PhaseKind) -> Self {
        Self::Phase(value)
    }
}

impl ScopeSet {
    pub(crate) fn iter(&self) -> impl Iterator<Item = Scope> {
        self.into_iter()
    }
}

impl IntoIterator for ScopeSet {
    type Item = Scope;
    type IntoIter = Box<dyn Iterator<Item = Scope>>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::All => Box::new([Self::Global, Self::Eclass, Self::Phases].iter().flatten()),
            Self::Eclass => Box::new([Scope::Eclass].into_iter()),
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

impl IntoIterator for &ScopeSet {
    type Item = Scope;
    type IntoIter = Box<dyn Iterator<Item = Scope>>;

    fn into_iter(self) -> Self::IntoIter {
        (*self).into_iter()
    }
}
