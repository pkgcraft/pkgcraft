use scallop::shell;

use crate::pkg::{ebuild::Pkg, BuildablePackage, Package};
use crate::pkgsh::{run_phase, source_ebuild, BuildData};

use super::Operation;

impl<'a> BuildablePackage for Pkg<'a> {
    fn build(&self) -> crate::Result<()> {
        BuildData::from_pkg(self);
        source_ebuild(self.path())?;

        shell::toggle_restricted(false);
        for phase in self.eapi().operation(Operation::Build) {
            run_phase(*phase)?;
        }
        shell::toggle_restricted(true);

        shell::reset();
        Ok(())
    }

    fn pretend(&self) -> crate::Result<()> {
        BuildData::from_pkg(self);
        source_ebuild(self.path())?;
        shell::toggle_restricted(false);
        for phase in self.eapi().operation(Operation::Pretend) {
            run_phase(*phase)?;
        }
        shell::toggle_restricted(true);
        shell::reset();
        Ok(())
    }
}
