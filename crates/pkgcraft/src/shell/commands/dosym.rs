use std::os::unix::fs::symlink;

use camino::{Utf8Path, Utf8PathBuf};
use scallop::{Error, ExecStatus};

use crate::eapi::Feature::DosymRelative;
use crate::shell::get_build_mut;
use crate::utils::relpath_utf8;

use super::{TryParseArgs, make_builtin};

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

#[derive(clap::Parser, Debug)]
#[command(
    name = "dosym",
    disable_help_flag = true,
    long_about = "Create symbolic links."
)]
struct Command {
    #[arg(short)]
    relative: bool,

    #[arg(allow_hyphen_values = true)]
    target: Utf8PathBuf,

    #[arg(allow_hyphen_values = true)]
    name: Utf8PathBuf,
}

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let mut target = cmd.target;
    let name = cmd.name;

    // convert target to relative path
    let eapi = get_build_mut().eapi();
    if cmd.relative && eapi.has(DosymRelative) {
        target = convert_target(&target, &name)?;
    }

    // check for unsupported dir target arg -- https://bugs.gentoo.org/379899
    if name.as_str().ends_with('/') || (name.is_dir() && !name.is_symlink()) {
        return Err(Error::Base(format!("missing filename target: {target}")));
    }

    get_build_mut()
        .install()
        .link(|p, q| symlink(p, q), target, name)?;

    Ok(ExecStatus::Success)
}

make_builtin!("dosym", dosym_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::eapi::EAPIS_OFFICIAL;
    use crate::shell::BuildData;
    use crate::shell::test::FileTree;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, functions::dosym};
    use super::*;

    cmd_scope_tests!("dosym path/to/source /path/to/target");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(dosym, &[0, 1, 4]);

        for eapi in EAPIS_OFFICIAL.iter().filter(|e| !e.has(DosymRelative)) {
            BuildData::empty(eapi);
            assert_invalid_cmd(dosym, &[3]);
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
