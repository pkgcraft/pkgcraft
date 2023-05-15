use scallop::shell;

use crate::pkg::{ebuild::Pkg, BuildablePackage, Package};
use crate::pkgsh::{source_ebuild, BuildData};

use super::Operation;

impl<'a> BuildablePackage for Pkg<'a> {
    fn build(&self) -> crate::Result<()> {
        BuildData::from_pkg(self);
        source_ebuild(self.path())?;

        for phase in self.eapi().operation(Operation::Build) {
            phase.run()?;
        }

        shell::reset(&["PATH"]);

        Ok(())
    }

    fn pretend(&self) -> crate::Result<()> {
        BuildData::from_pkg(self);
        source_ebuild(self.path())?;

        for phase in self.eapi().operation(Operation::Pretend) {
            phase.run()?;
        }

        shell::reset(&["PATH"]);

        Ok(())
    }
}
