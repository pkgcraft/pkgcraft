use std::fs;
use std::os::fd::AsRawFd;

use scallop::pool::redirect_output;
use scallop::{Error, ExecStatus};
use tempfile::NamedTempFile;

use crate::error::PackageError;
use crate::pkg::{ebuild, Build, Package, Pretend, Regen, Source};
use crate::shell::metadata::Metadata;
use crate::shell::{get_build_mut, BuildData};

use super::OperationKind;

impl<'a> Build for ebuild::Pkg<'a> {
    fn build(&self) -> scallop::Result<()> {
        get_build_mut()
            .source_ebuild(&self.abspath())
            .map_err(|e| self.invalid_pkg_err(e))?;

        for phase in self.eapi().operation(OperationKind::Build)? {
            phase.run().map_err(|e| self.pkg_err(e))?;
        }

        Ok(())
    }
}

impl<'a> Pretend for ebuild::Pkg<'a> {
    fn pretend(&self) -> scallop::Result<Option<String>> {
        // ignore packages with EAPIs lacking pkg_pretend() support
        if let Ok(op) = self.eapi().operation(OperationKind::Pretend) {
            let phase = op.phases[0];
            self.source()?;

            // redirect pkg_pretend() output to a temporary file
            let file = NamedTempFile::new()?;
            redirect_output(file.as_raw_fd())?;

            let result = phase.run();
            let output = fs::read_to_string(file.path()).unwrap_or_default();
            let output = output.trim();
            if let Err(e) = result {
                if output.is_empty() {
                    return Err(Error::Base(format!("{self}: {e}")));
                } else {
                    return Err(Error::Base(format!("{self}: {e}\n{output}")));
                }
            } else if !output.is_empty() {
                return Ok(Some(format!("{self}\n{output}")));
            }
        }

        Ok(None)
    }
}

impl<'a> Source for ebuild::raw::Pkg<'a> {
    fn source(&self) -> scallop::Result<ExecStatus> {
        BuildData::from_raw_pkg(self);
        get_build_mut().source_ebuild(self.data())
    }
}

impl<'a> Source for ebuild::Pkg<'a> {
    fn source(&self) -> scallop::Result<ExecStatus> {
        BuildData::from_pkg(self);
        get_build_mut().source_ebuild(&self.abspath())
    }
}

impl<'a> Regen for ebuild::raw::Pkg<'a> {
    fn regen(&self) -> scallop::Result<()> {
        Ok(Metadata::serialize(self).map_err(|e| self.invalid_pkg_err(e))?)
    }
}
