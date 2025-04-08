use std::os::unix::fs::symlink;

use camino::{Utf8Path, Utf8PathBuf};
use scallop::{Error, ExecStatus};

use crate::eapi::Feature::DosymRelative;
use crate::shell::get_build_mut;
use crate::utils::relpath_utf8;

use super::make_builtin;

const LONG_DOC: &str = "Create symbolic links.";

/// Convert link target from an absolute path to a path relative to its name.
fn convert_target<P: AsRef<Utf8Path>>(target: P, name: P) -> scallop::Result<Utf8PathBuf> {
    let target = target.as_ref();
    let name = name.as_ref();

    if !target.is_absolute() {
        return Err(Error::Base(format!("absolute path required with '-r': {target}")));
    }

    let parent = name.parent().map(|x| x.as_str()).unwrap_or("/");
    relpath_utf8(target, parent)
        .ok_or_else(|| Error::Base(format!("invalid relative path: {target} -> {name}")))
}

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let eapi = get_build_mut().eapi();
    let (target, name) = match args[..] {
        ["-r", target, name] if eapi.has(DosymRelative) => {
            (convert_target(target, name)?, name)
        }
        [target, name] => (Utf8PathBuf::from(target), name),
        _ => return Err(Error::Base(format!("requires 2 args, got {}", args.len()))),
    };

    // check for unsupported dir target arg -- https://bugs.gentoo.org/379899
    let name_path = Utf8Path::new(name);
    if name.ends_with('/') || (name_path.is_dir() && !name_path.is_symlink()) {
        return Err(Error::Base(format!("missing filename target: {target}")));
    }

    get_build_mut()
        .install()
        .link(|p, q| symlink(p, q), target, name)?;

    Ok(ExecStatus::Success)
}

make_builtin!("dosym", dosym_builtin, true);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::eapi::EAPIS_OFFICIAL;
    use crate::shell::test::FileTree;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_args, cmd_scope_tests, dosym};
    use super::*;

    cmd_scope_tests!("dosym path/to/source /path/to/target");

    #[test]
    fn invalid_args() {
        assert_invalid_args(dosym, &[0, 1, 4]);

        for eapi in EAPIS_OFFICIAL.iter().filter(|e| !e.has(DosymRelative)) {
            BuildData::empty(eapi);
            assert_invalid_args(dosym, &[3]);
        }
    }

    #[test]
    fn errors() {
        let _file_tree = FileTree::new();

        // dir targets aren't supported
        let r = dosym(&["source", "target/"]);
        assert_err_re!(r, "^missing filename target: .*$");

        fs::create_dir("target").unwrap();
        let r = dosym(&["source", "target"]);
        assert_err_re!(r, "^missing filename target: .*$");

        // linking to the root directory isn't supported
        let r = dosym(&["-r", "/source", "/"]);
        assert_err_re!(r, "^missing filename target: .*$");

        // relative source with `dosym -r`
        let r = dosym(&["-r", "source", "target"]);
        assert_err_re!(r, "^absolute path required .*$");
    }

    #[test]
    fn linking() {
        let file_tree = FileTree::new();

        dosym(&["/usr/bin/source", "target"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/target"
            link = "/usr/bin/source"
        "#,
        );

        dosym(&["-r", "/usr/bin/source", "/usr/bin/target"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/bin/target"
            link = "source"
        "#,
        );
    }
}
