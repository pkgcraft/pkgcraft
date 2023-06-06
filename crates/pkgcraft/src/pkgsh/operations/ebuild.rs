use std::fs;
use std::os::fd::AsRawFd;

use scallop::pool::redirect_output;
use tempfile::NamedTempFile;

use crate::error::{Error, PackageError};
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

        for phase in self.eapi().operation(Operation::Build)? {
            phase.run().map_err(|e| self.pkg_err(e))?;
        }

        Ok(())
    }

    fn pretend(&self) -> scallop::Result<()> {
        // ignore packages lacking pkg_pretend() support
        if let Ok(phases) = self.eapi().operation(Operation::Pretend) {
            BuildData::from_pkg(self);
            get_build_mut()
                .source_ebuild(self.path())
                .map_err(|e| self.invalid_pkg_err(e))?;

            // redirect pkg_pretend() output to a temporary file
            let file = NamedTempFile::new()?;
            redirect_output(file.as_raw_fd())?;

            for phase in phases {
                phase.run().map_err(|e| {
                    // get redirected output
                    let output = fs::read_to_string(file.path()).unwrap_or_default();
                    let output = output.trim();

                    // determine error string
                    let err = if output.is_empty() {
                        e.to_string()
                    } else {
                        format!("{e}\n{output}")
                    };

                    Error::Pkg { id: self.to_string(), err }
                })?;
            }
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

    fn metadata(&self) -> scallop::Result<()> {
        // source package and generate metadata
        let meta = Metadata::source(self)?;

        // serialize metadata to disk
        meta.serialize(self).map_err(|e| self.pkg_err(e))?;

        Ok(())
    }
}
