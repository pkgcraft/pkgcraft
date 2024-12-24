mod fetch;
mod manifest;
mod pretend;
mod showkw;
mod source;

macro_rules! cmd_arg_tests {
    ($cmd:expr) => {
        #[test]
        fn invalid_cwd_target() {
            let dir = tempfile::tempdir().unwrap();
            std::env::set_current_dir(dir.path()).unwrap();
            let s = "invalid ebuild repo: .";
            pkgcraft::test::cmd($cmd)
                .assert()
                .stdout("")
                .stderr(predicates::str::contains(s))
                .failure();
        }

        #[test]
        fn nonexistent_path_target() {
            let repo = "path/to/nonexistent/repo";
            let cmd = format!("{} {repo}", $cmd);
            let s = format!("invalid path target: {repo}: No such file or directory");
            pkgcraft::test::cmd(cmd)
                .assert()
                .stdout("")
                .stderr(predicates::str::contains(s))
                .failure();
        }

        #[test]
        fn empty_repo() {
            let data = pkgcraft::test::test_data();
            let repo = data.ebuild_repo("empty").unwrap();
            pkgcraft::test::cmd($cmd)
                .arg(repo)
                .assert()
                .stdout("")
                .stderr("")
                .success();
        }

        #[test]
        fn no_matches() {
            let cmd = format!("{} cat/pkg", $cmd);
            let s = "no matches found: cat/pkg";
            pkgcraft::test::cmd(cmd)
                .assert()
                .stdout("")
                .stderr(predicates::str::contains(s))
                .failure()
                .code(2);
        }
    };
}
use cmd_arg_tests;
