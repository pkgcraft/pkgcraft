use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;

use super::{eapply, make_builtin};

const LONG_DOC: &str = "Apply user patches.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
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
make_builtin!("eapply_user", eapply_user_builtin);

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use tempfile::tempdir;

    use crate::shell::BuildData;
    use crate::test::assert_err_re;
    use crate::test::test_data;

    use super::super::{assert_invalid_args, cmd_scope_tests, eapply_user};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(eapply_user, &[1]);
    }

    #[test]
    fn failure() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        get_build_mut().user_patches = ["file.patch".to_string()].into_iter().collect();

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
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        get_build_mut().user_patches = ["files/0.patch".to_string(), "files/1.patch".to_string()]
            .into_iter()
            .collect();

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
