use crate::error::PackageError;
use crate::pkg::ebuild::{Pkg, RawPkg};
use crate::pkg::{BuildablePackage, Package, SourceablePackage};
use crate::pkgsh::metadata::Metadata;
use crate::pkgsh::{get_build_mut, BuildData};

use super::Operation;

impl<'a> BuildablePackage for Pkg<'a> {
    fn build(&self) -> scallop::Result<()> {
        get_build_mut()
            .source_ebuild(self.path())
            .map_err(|e| self.invalid_pkg_err(e))?;

        for phase in self.eapi().operation(Operation::Build) {
            phase.run().map_err(|e| self.pkg_err(e))?;
        }

        Ok(())
    }

    fn pretend(&self) -> scallop::Result<()> {
        BuildData::from_pkg(self);
        get_build_mut()
            .source_ebuild(self.path())
            .map_err(|e| self.invalid_pkg_err(e))?;

        for phase in self.eapi().operation(Operation::Pretend) {
            phase.run().map_err(|e| self.pkg_err(e))?;
        }
        Ok(())
    }
}

impl<'a> SourceablePackage for RawPkg<'a> {
    fn source(&self) -> scallop::Result<()> {
        BuildData::from_raw_pkg(self);
        get_build_mut()
            .source_ebuild(self.data())
            .map_err(|e| self.invalid_pkg_err(e))?;
        Ok(())
    }

    fn metadata(&self, force: bool) -> scallop::Result<()> {
        // verify metadata validity using ebuild and eclass hashes
        if !force && Metadata::valid(self) {
            return Ok(());
        }

        // source package and generate metadata
        let meta = Metadata::source(self)?;

        // serialize metadata to disk
        meta.serialize(self).map_err(|e| self.pkg_err(e))?;

        Ok(())
    }
}
