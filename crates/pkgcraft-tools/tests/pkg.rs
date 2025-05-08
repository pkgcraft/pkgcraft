mod env;
mod fetch;
mod manifest;
mod metadata;
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
            // no repos
            let s = "no repos available";
            pkgcraft::test::cmd($cmd)
                .arg("cat/pkg")
                .assert()
                .stdout("")
                .stderr(predicates::str::contains(s))
                .failure()
                .code(2);

            let data = pkgcraft::test::test_data();
            let repo = data.ebuild_repo("empty").unwrap();

            // Cpn target
            let s = "no matches found: cat/pkg";
            pkgcraft::test::cmd($cmd)
                .args(["-r", repo.path().as_str()])
                .arg("cat/pkg")
                .assert()
                .stdout("")
                .stderr(predicates::str::contains(s))
                .failure()
                .code(2);

            // category target
            let s = "no matches found: category";
            pkgcraft::test::cmd($cmd)
                .args(["-r", repo.path().as_str()])
                .arg("category")
                .assert()
                .stdout("")
                .stderr(predicates::str::contains(s))
                .failure()
                .code(2);
        }
    };
}
use cmd_arg_tests;
