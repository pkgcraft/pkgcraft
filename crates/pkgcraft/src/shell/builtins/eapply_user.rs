use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;
use crate::shell::phase::PhaseKind::SrcPrepare;

use super::{eapply::run as eapply, make_builtin};

const LONG_DOC: &str = "Apply user patches.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Base(format!("takes no args, got {}", args.len())));
    }

    let build = get_build_mut();

    if !build.user_patches_applied {
        let args: Vec<_> = build.user_patches.iter().map(|s| s.as_str()).collect();
        if !args.is_empty() {
            eapply(&args)?;
        }

        build.user_patches_applied = true;
    }

    Ok(ExecStatus::Success)
}

const USAGE: &str = "eapply_user";
make_builtin!("eapply_user", eapply_user_builtin, run, LONG_DOC, USAGE, [("6..", [SrcPrepare])]);

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use tempfile::tempdir;

    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::shell::BuildData;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as eapply_user;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(eapply_user, &[1]);
    }

    #[test]
    fn failure() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_pkg(&pkg);
        get_build_mut().user_patches = vec!["file.patch".to_string()];

        let file_content = indoc::indoc! {"
            1
        "};
        let bad_content = indoc::indoc! {"
            --- a/file.txt
            +++ a/file.txt
            @@ -1 +1 @@
            -2
            +3
        "};
        let bad_prefix = indoc::indoc! {"
            --- a/b/file.txt
            +++ a/b/file.txt
            @@ -1 +1 @@
            -1
            +2
        "};

        let dir = tempdir().unwrap();
        env::set_current_dir(&dir).unwrap();
        fs::write("file.txt", file_content).unwrap();
        for data in [bad_content, bad_prefix] {
            fs::write("file.patch", data).unwrap();
            let r = eapply_user(&[]);
            assert_err_re!(r, "^failed applying: file.patch");
        }
    }

    #[test]
    fn success() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_pkg(&pkg);
        get_build_mut().user_patches =
            vec!["files/0.patch".to_string(), "files/1.patch".to_string()];

        let file_content = indoc::indoc! {"
            0
        "};
        let patch0 = indoc::indoc! {"
            --- a/file.txt
            +++ a/file.txt
            @@ -1 +1 @@
            -0
            +1
        "};
        let patch1 = indoc::indoc! {"
            --- a/file.txt
            +++ a/file.txt
            @@ -1 +1 @@
            -1
            +2
        "};

        let dir = tempdir().unwrap();
        env::set_current_dir(&dir).unwrap();
        fs::write("file.txt", file_content).unwrap();
        fs::create_dir("files").unwrap();
        for (i, data) in [patch0, patch1].iter().enumerate() {
            let file = format!("files/{i}.patch");
            fs::write(file, data).unwrap();
        }
        assert_eq!(fs::read_to_string("file.txt").unwrap(), "0\n");
        eapply_user(&[]).unwrap();
        assert_eq!(fs::read_to_string("file.txt").unwrap(), "2\n");
        // re-running doesn't apply patches
        eapply_user(&[]).unwrap();
    }
}
