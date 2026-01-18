use std::io::{Read, Seek};

use scallop::pool::redirect_output;
use scallop::{Error, ExecStatus, functions};
use tempfile::tempfile;

use crate::pkg::ebuild::{EbuildPkg, EbuildRawPkg};
use crate::pkg::{Build, Package, PkgPretend, Source};
use crate::shell::phase::PhaseKind;
use crate::shell::{BuildData, get_build_mut};

use super::OperationKind;

impl Build for EbuildPkg {
    fn build(&self) -> scallop::Result<()> {
        let build = get_build_mut();

        build.source_ebuild(&self.path()).map_err(|e| {
            let err: crate::Error = e.into();
            err.into_invalid_pkg_err(self)
        })?;

        build.create_dirs()?;

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

        // source ebuild
        self.source()?;

        // find native function
        let Some(mut func) = functions::find(phase) else {
            return Err(Error::Base(format!("{self}: {phase} phase missing")));
        };

        // initialize build scope
        let build = get_build_mut();
        build.scope = phase.kind.into();

        // initialize phase scope variables
        build.set_vars()?;

        // redirect pkg_pretend() output to a temporary file
        let mut file =
            tempfile().map_err(|e| Error::IO(format!("failed creating tempfile: {e}")))?;
        redirect_output(&file)?;

        // run phase
        let result = func.execute(&[]);

        // read output from temporary file
        let mut output = vec![];
        file.rewind()?;
        file.read_to_end(&mut output)?;
        // replace invalid UTF-8 in output
        let output = String::from_utf8_lossy(&output);
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
        let r = pkg.pretend();
        assert!(r.is_ok(), "failed running pkg_pretend: {}", r.unwrap_err());
        assert!(r.unwrap().is_none());

        // success
        let pkg = repo.get_pkg("pkg-pretend/success-1").unwrap();
        let r = pkg.pretend();
        assert!(r.is_ok(), "failed running pkg_pretend: {}", r.unwrap_err());
        assert!(r.unwrap().is_none());

        // success with output
        let pkg = repo.get_pkg("pkg-pretend/success-with-output-1").unwrap();
        let r = pkg.pretend();
        assert!(r.is_ok(), "failed running pkg_pretend: {}", r.unwrap_err());
        let output = r.unwrap().unwrap();
        let output = output.lines().nth(1).unwrap();
        assert_eq!(output, "output123");

        // failure
        let pkg = repo.get_pkg("pkg-pretend/failure-1").unwrap();
        let r = pkg.pretend();
        assert!(r.is_err());
        let output = r.unwrap_err().to_string();
        assert!(!output.contains("output123"));

        // failure with output
        let pkg = repo.get_pkg("pkg-pretend/failure-with-output-1").unwrap();
        let r = pkg.pretend();
        assert!(r.is_err());
        let output = r.unwrap_err().to_string();
        let output = output.lines().nth(1).unwrap();
        assert_eq!(output, "output123");
    }
}
