use std::fs;
use std::os::fd::AsRawFd;

use scallop::pool::redirect_output;
use scallop::ExecStatus;
use tempfile::NamedTempFile;

use crate::error::{Error, PackageError};
use crate::pkg::{ebuild, BuildPackage, Package, PackageMetadata, SourcePackage};
use crate::shell::metadata::Metadata;
use crate::shell::{get_build_mut, BuildData};

use super::OperationKind::{Build, Pretend};

impl<'a> BuildPackage for ebuild::Pkg<'a> {
    fn build(&self) -> scallop::Result<()> {
        get_build_mut()
            .source_ebuild(&self.abspath())
            .map_err(|e| self.invalid_pkg_err(e))?;

        for phase in self.eapi().operation(Build)? {
            phase.run().map_err(|e| self.pkg_err(e))?;
        }

        Ok(())
    }

    fn pretend(&self) -> scallop::Result<()> {
        // ignore packages lacking pkg_pretend() support
        if let Ok(phases) = self.eapi().operation(Pretend) {
            self.source()?;

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

impl<'a> SourcePackage for ebuild::raw::Pkg<'a> {
    fn source(&self) -> scallop::Result<ExecStatus> {
        BuildData::from_raw_pkg(self);
        get_build_mut().source_ebuild(self.data())
    }
}

impl<'a> SourcePackage for ebuild::Pkg<'a> {
    fn source(&self) -> scallop::Result<ExecStatus> {
        BuildData::from_pkg(self);
        get_build_mut().source_ebuild(&self.abspath())
    }
}

impl<'a> PackageMetadata for ebuild::raw::Pkg<'a> {
    fn metadata(&self) -> scallop::Result<()> {
        Ok(Metadata::serialize(self).map_err(|e| self.invalid_pkg_err(e))?)
    }
}
