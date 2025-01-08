use camino::Utf8PathBuf;
use pkgcraft::restrict::Scope;

use crate::report::ReportScope;

pub struct IgnorePaths<'a> {
    target: &'a ReportScope,
    scope: Option<Scope>,
}

impl<'a> IgnorePaths<'a> {
    pub fn new(target: &'a ReportScope) -> Self {
        Self {
            target,
            scope: Some(Scope::Repo),
        }
    }
}

impl Iterator for IgnorePaths<'_> {
    type Item = (Scope, Utf8PathBuf);

    fn next(&mut self) -> Option<Self::Item> {
        self.scope.map(|scope| {
            // construct the relative path to check for ignore files
            let relpath = match (scope, self.target) {
                (Scope::Category, ReportScope::Category(category)) => category.into(),
                (Scope::Category, ReportScope::Package(cpn)) => cpn.category().into(),
                (Scope::Category, ReportScope::Version(cpv, _)) => cpv.category().into(),
                (Scope::Package, ReportScope::Package(cpn)) => cpn.to_string().into(),
                (Scope::Package, ReportScope::Version(cpv, _)) => cpv.cpn().to_string().into(),
                (Scope::Version, ReportScope::Version(cpv, _)) => cpv.relpath(),
                _ => Default::default(),
            };

            // set the scope to the next lower level
            self.scope = match scope {
                Scope::Repo => Some(Scope::Category),
                Scope::Category => Some(Scope::Package),
                Scope::Package => Some(Scope::Version),
                Scope::Version => None,
            };

            (scope, relpath)
        })
    }
}
