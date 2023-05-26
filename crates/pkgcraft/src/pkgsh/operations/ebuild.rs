use scallop::Error;

use crate::pkg::ebuild::{Pkg, RawPkg};
use crate::pkg::{BuildablePackage, Package, SourceablePackage};
use crate::pkgsh::metadata::Metadata;
use crate::pkgsh::{get_build_mut, BuildData};

use super::Operation;

impl<'a> BuildablePackage for Pkg<'a> {
    fn build(&self) -> scallop::Result<()> {
        get_build_mut()
            .source_ebuild(self.path())
            .map_err(|e| Error::Base(format!("{self}: {e}")))?;

        for phase in self.eapi().operation(Operation::Build) {
            phase
                .run()
                .map_err(|e| Error::Base(format!("{self}: {e}")))?;
        }

        Ok(())
    }

    fn pretend(&self) -> scallop::Result<()> {
        BuildData::from_pkg(self);
        get_build_mut()
            .source_ebuild(self.path())
            .map_err(|e| Error::Base(format!("{self}: {e}")))?;

        for phase in self.eapi().operation(Operation::Pretend) {
            phase
                .run()
                .map_err(|e| Error::Base(format!("{self}: {e}")))?;
        }
        Ok(())
    }
}

impl<'a> SourceablePackage for RawPkg<'a> {
    fn source(&self) -> scallop::Result<()> {
        BuildData::from_raw_pkg(self);
        get_build_mut()
            .source_ebuild(self.data())
            .map_err(|e| Error::Base(format!("{self}: {e}")))?;
        Ok(())
    }

    fn metadata(&self) -> scallop::Result<()> {
        let _meta = Metadata::source(self).map_err(|e| Error::Base(format!("{self}: {e}")))?;
        // TODO: serialize to metadata/md5-cache
        Ok(())
    }
}
