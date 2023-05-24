use crate::pkg::{ebuild::Pkg, BuildablePackage, Package};
use crate::pkgsh::{get_build_mut, BuildData};

use super::Operation;

impl<'a> BuildablePackage for Pkg<'a> {
    fn metadata(&self) -> crate::Result<()> {
        BuildData::from_pkg(self);
        get_build_mut().source_ebuild(self.path())?;
        // TODO: serialize to metadata/md5-cache
        Ok(())
    }

    fn build(&self) -> crate::Result<()> {
        get_build_mut().source_ebuild(self.path())?;

        for phase in self.eapi().operation(Operation::Build) {
            phase.run()?;
        }

        Ok(())
    }

    fn pretend(&self) -> crate::Result<()> {
        BuildData::from_pkg(self);
        get_build_mut().source_ebuild(self.path())?;

        for phase in self.eapi().operation(Operation::Pretend) {
            phase.run()?;
        }

        Ok(())
    }
}
