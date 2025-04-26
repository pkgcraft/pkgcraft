use std::fs;

use scallop::pool::redirect_output;
use scallop::{Error, ExecStatus, functions};
use tempfile::NamedTempFile;

use crate::pkg::ebuild::{EbuildPkg, EbuildRawPkg};
use crate::pkg::{Build, Package, PkgPretend, Source};
use crate::shell::phase::PhaseKind;
use crate::shell::scope::Scope;
use crate::shell::{BuildData, get_build_mut};

use super::OperationKind;

impl Build for EbuildPkg {
    fn build(&self) -> scallop::Result<()> {
        get_build_mut().source_ebuild(&self.path()).map_err(|e| {
            let err: crate::Error = e.into();
            err.into_invalid_pkg_err(self)
        })?;

        for phase in self.eapi().operation(OperationKind::Build) {
            phase.run().map_err(|e| {
                let err: crate::Error = e.into();
                err.into_pkg_err(self)
            })?;
        }

        Ok(())
    }
}

impl PkgPretend for EbuildPkg {
    fn pkg_pretend(&self) -> scallop::Result<Option<String>> {
        let Some(phase) = self.eapi().phases().get(&PhaseKind::PkgPretend) else {
            // ignore packages with EAPIs lacking pkg_pretend() support
            return Ok(None);
        };

        if !self.defined_phases().contains(&phase.kind) {
            // phase function is undefined
            return Ok(None);
        }

        self.source()?;

        let Some(mut func) = functions::find(phase) else {
            return Err(Error::Base(format!("{self}: {phase} phase missing")));
        };

        let build = get_build_mut();
        build.scope = Scope::Phase(phase.kind);

        // initialize phase scope variables
        build.set_vars()?;

        // redirect pkg_pretend() output to a temporary file
        let file = NamedTempFile::new()?;
        redirect_output(&file)?;

        // execute function capturing output
        let result = func.execute(&[]);
        let output = fs::read_to_string(file.path()).unwrap_or_default();
        let output = output.trim();

        if let Err(e) = result {
            if output.is_empty() {
                Err(Error::Base(format!("{self}: {e}")))
            } else {
                Err(Error::Base(format!("{self}: {e}\n{output}")))
            }
        } else if !output.is_empty() {
            Ok(Some(format!("{self}\n{output}")))
        } else {
            Ok(None)
        }
    }
}

impl Source for EbuildRawPkg {
    fn source(&self) -> scallop::Result<ExecStatus> {
        BuildData::from_raw_pkg(self);
        get_build_mut().source_ebuild(self.data())
    }
}

impl Source for EbuildPkg {
    fn source(&self) -> scallop::Result<ExecStatus> {
        BuildData::from_pkg(self);
        get_build_mut().source_ebuild(&self.path())
    }
}

#[cfg(test)]
mod tests {
    use crate::test::test_data;

    #[test]
    fn pkg_pretend() {
        let data = test_data();
        let repo = data.ebuild_repo("phases").unwrap();

        // no pkg_pretend phase exists
        let pkg = repo.get_pkg("pkg-pretend/none-1").unwrap();
        assert!(pkg.pretend().is_ok());

        // success
        let pkg = repo.get_pkg("pkg-pretend/success-1").unwrap();
        assert!(pkg.pretend().is_ok());

        // success with output
        let pkg = repo.get_pkg("pkg-pretend/success-with-output-1").unwrap();
        assert!(pkg.pretend().is_ok());

        // failure
        let pkg = repo.get_pkg("pkg-pretend/failure-1").unwrap();
        assert!(pkg.pretend().is_err());

        // failure with output
        let pkg = repo.get_pkg("pkg-pretend/failure-with-output-1").unwrap();
        assert!(pkg.pretend().is_err());
    }
}
