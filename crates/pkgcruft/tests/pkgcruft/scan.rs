use std::env;

use pkgcraft::test::{assert_unordered_eq, cmd, test_data_path};
use pkgcruft::test::*;
use predicates::prelude::*;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use tempfile::{tempdir, NamedTempFile};

#[test]
fn stdin_targets() {
    let repo = test_data_path().join("repos/valid/qa-primary");

    for arg in ["KeywordsDropped", "KeywordsDropped/KeywordsDropped"] {
        cmd("pkgcruft scan -R simple -")
            .args(["--repo", repo.as_ref()])
            .write_stdin(format!("{arg}\n"))
            .assert()
            .stdout(contains("KeywordsDropped: x86"))
            .stderr("")
            .success();
    }
}

#[test]
fn dep_restrict_targets() {
    let repo = test_data_path().join("repos/valid/qa-primary");

    // invalid
    for s in ["^pkg", "cat&pkg"] {
        cmd("pkgcruft scan")
            .arg(s)
            .assert()
            .stdout("")
            .stderr(contains(format!("invalid dep restriction: {s}")))
            .failure()
            .code(2);
    }

    // single
    for s in ["KeywordsDropped/*", "KeywordsDropped"] {
        cmd("pkgcruft scan -R simple")
            .args(["--repo", repo.as_ref()])
            .arg(s)
            .assert()
            .stdout(contains("KeywordsDropped: x86"))
            .stderr("")
            .success();
    }

    // multiple matching restricts output the same reports
    let reports = indoc::indoc! {r#"
        KeywordsDropped/KeywordsDropped-2: KeywordsDropped: x86
        KeywordsDropped/KeywordsDropped-2: KeywordsDropped: x86
    "#};
    let expected: Vec<_> = reports.lines().collect();
    let output = cmd("pkgcruft scan -R simple")
        .args(["--repo", repo.as_ref()])
        .args(["KeywordsDropped/*", "KeywordsDropped"])
        .output()
        .unwrap()
        .stdout;
    let data = String::from_utf8(output).unwrap();
    let data: Vec<_> = data.lines().collect();
    assert_unordered_eq!(&expected, &data);

    // nonexistent
    cmd("pkgcruft scan -R simple")
        .args(["--repo", repo.as_ref()])
        .arg("nonexistent/pkg")
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn current_dir_targets() {
    let primary_repo = test_data_path().join("repos/valid/qa-primary");
    let secondary_repo = test_data_path().join("repos/valid/qa-secondary");

    // empty dir
    let tmpdir = tempdir().unwrap();
    let path = tmpdir.path().to_str().unwrap();
    env::set_current_dir(path).unwrap();
    cmd("pkgcruft scan")
        .assert()
        .stdout("")
        .stderr(contains(format!("invalid ebuild repo: {path}")))
        .failure()
        .code(2);

    // repo dir
    env::set_current_dir(&primary_repo).unwrap();
    let expected = glob_reports!("{primary_repo}/**/reports.json");
    let reports = cmd("pkgcruft scan -R json").to_reports().unwrap();
    assert_unordered_eq!(&expected, &reports);

    env::set_current_dir(&secondary_repo).unwrap();
    let expected = glob_reports!("{secondary_repo}/**/reports.json");
    let reports = cmd("pkgcruft scan -R json").to_reports().unwrap();
    assert_unordered_eq!(&expected, &reports);

    // category dir
    env::set_current_dir(primary_repo.join("Dependency")).unwrap();
    let expected = glob_reports!("{primary_repo}/Dependency/**/reports.json");
    let reports = cmd("pkgcruft scan -R json").to_reports().unwrap();
    assert_unordered_eq!(&expected, &reports);

    env::set_current_dir(secondary_repo.join("Overlay")).unwrap();
    let expected = glob_reports!("{secondary_repo}/Overlay/**/reports.json");
    let reports = cmd("pkgcruft scan -R json").to_reports().unwrap();
    assert_unordered_eq!(&expected, &reports);

    // package dir
    env::set_current_dir(primary_repo.join("Dependency/DependencyDeprecated")).unwrap();
    let expected = glob_reports!("{primary_repo}/Dependency/DependencyDeprecated/reports.json");
    let reports = cmd("pkgcruft scan -R json").to_reports().unwrap();
    assert_unordered_eq!(&expected, &reports);

    env::set_current_dir(secondary_repo.join("Overlay/EclassUnused")).unwrap();
    let expected = glob_reports!("{secondary_repo}/Overlay/EclassUnused/reports.json");
    let reports = cmd("pkgcruft scan -R json").to_reports().unwrap();
    assert_unordered_eq!(&expected, &reports);
}

#[test]
fn path_targets() {
    let repo = test_data_path().join("repos/valid/qa-primary");

    // nonexistent
    cmd("pkgcruft scan path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(contains("invalid path target"))
        .failure()
        .code(2);

    // invalid
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_str().unwrap();
    cmd("pkgcruft scan")
        .arg(path)
        .assert()
        .stdout("")
        .stderr(contains(format!("invalid ebuild repo: {path}")))
        .failure()
        .code(2);

    // unsupported EAPI
    let path = test_data_path().join("repos/invalid/unsupported-eapi");
    cmd("pkgcruft scan")
        .arg(&path)
        .assert()
        .stdout("")
        .stderr(contains("unsupported EAPI: 0"))
        .failure()
        .code(2);

    // repo dir
    let expected = glob_reports!("{repo}/**/reports.json");
    let reports = cmd("pkgcruft scan -R json")
        .arg(&repo)
        .to_reports()
        .unwrap();
    assert_unordered_eq!(&expected, &reports);

    // non-package dir
    let reports = cmd("pkgcruft scan -R json")
        .arg(repo.join("licenses"))
        .to_reports()
        .unwrap();
    assert_unordered_eq!(&reports, &[]);

    // TODO: test overlay dir

    // category dir
    let expected = glob_reports!("{repo}/Dependency/**/reports.json");
    let reports = cmd("pkgcruft scan -R json")
        .arg(repo.join("Dependency"))
        .to_reports()
        .unwrap();
    assert_unordered_eq!(&expected, &reports);

    // package dir
    let expected = glob_reports!("{repo}/Dependency/DependencyDeprecated/reports.json");
    let reports = cmd("pkgcruft scan -R json")
        .arg(repo.join("Dependency/DependencyDeprecated"))
        .to_reports()
        .unwrap();
    assert_unordered_eq!(&expected, &reports);

    // ebuild file
    let reports = cmd("pkgcruft scan -R json")
        .arg(repo.join("Dependency/DependencyDeprecated/DependencyDeprecated-0.ebuild"))
        .to_reports()
        .unwrap();
    assert_unordered_eq!(&expected[..1], &reports);

    // multiple absolute paths in the same repo
    let expected = glob_reports!(
        "{repo}/Dependency/**/reports.json",
        "{repo}/EapiStatus/**/reports.json",
        "{repo}/Keywords/**/reports.json",
    );

    let reports = cmd("pkgcruft scan -R json")
        .arg(repo.join("Dependency"))
        .arg(repo.join("EapiStatus"))
        .arg(repo.join("Keywords"))
        .to_reports()
        .unwrap();
    assert_unordered_eq!(&expected, &reports);

    // multiple relative paths in the same repo
    env::set_current_dir(&repo).unwrap();
    let reports = cmd("pkgcruft scan -R json")
        .arg("Dependency")
        .arg("EapiStatus")
        .arg("Keywords")
        .to_reports()
        .unwrap();
    assert_unordered_eq!(&expected, &reports);
}

#[test]
fn color() {
    let repo = test_data_path().join("repos/valid/qa-primary");

    // forcibly disable color
    let reports = indoc::indoc! {"
        KeywordsDropped/KeywordsDropped
          KeywordsDropped: version 2: x86
    "};
    let expected: Vec<_> = reports.lines().collect();
    let output = cmd("pkgcruft scan --color false")
        .args(["--repo", repo.as_ref()])
        .arg("KeywordsDropped")
        .output()
        .unwrap()
        .stdout;
    let data = String::from_utf8(output).unwrap();
    let data: Vec<_> = data.lines().collect();
    assert_eq!(&expected, &data);

    // forcibly enable color
    let reports = indoc::indoc! {"
        \u{1b}[1;34mKeywordsDropped/KeywordsDropped\u{1b}[0m
          \u{1b}[33mKeywordsDropped\u{1b}[0m: version 2: x86
    "};
    let expected: Vec<_> = reports.lines().collect();
    for opts in ["--color true", "--color"] {
        let output = cmd(format!("pkgcruft scan {opts}"))
            .args(["--repo", repo.as_ref()])
            .arg("KeywordsDropped")
            .output()
            .unwrap()
            .stdout;
        let data = String::from_utf8(output).unwrap();
        let data: Vec<_> = data.lines().collect();
        assert_eq!(&expected, &data);
    }
}

#[test]
fn jobs() {
    let repo = test_data_path().join("repos/valid/qa-primary");
    let expected = glob_reports!("{repo}/**/reports.json");

    for opt in ["-j", "--jobs"] {
        // serialized scans run in order
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "1"])
            .arg(&repo)
            .to_reports()
            .unwrap();
        assert_eq!(&expected, &reports);
    }
}

#[test]
fn repo() {
    let repo = test_data_path().join("repos/valid/qa-primary");

    env::set_current_dir(&repo).unwrap();
    for path in [".", "./", repo.as_ref()] {
        // implicit repo target
        let reports = cmd("pkgcruft scan -R json")
            .args(["--repo", path])
            .to_reports()
            .unwrap();
        let expected = glob_reports!("{repo}/**/reports.json");
        assert_unordered_eq!(&expected, &reports);

        // category target
        let reports = cmd("pkgcruft scan -R json")
            .args(["--repo", path])
            .arg("Dependency/*")
            .to_reports()
            .unwrap();
        let expected = glob_reports!("{repo}/Dependency/**/reports.json");
        assert_unordered_eq!(&expected, &reports);

        // package target
        let reports = cmd("pkgcruft scan -R json")
            .args(["--repo", path])
            .arg("DependencyDeprecated")
            .to_reports()
            .unwrap();
        let expected = glob_reports!("{repo}/Dependency/DependencyDeprecated/reports.json");
        assert_unordered_eq!(&expected, &reports);

        // Cpn target
        let reports = cmd("pkgcruft scan -R json")
            .args(["--repo", path])
            .arg("Dependency/DependencyDeprecated")
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&expected, &reports);

        // Cpv target
        let reports = cmd("pkgcruft scan -R json")
            .args(["--repo", path])
            .arg("Dependency/DependencyDeprecated-0")
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&expected[..1], &reports);

        // P target
        let reports = cmd("pkgcruft scan -R json")
            .args(["--repo", path])
            .arg("DependencyDeprecated-0")
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&expected[..1], &reports);
    }

    // TODO: test overlay
}

#[test]
fn exit() {
    let repo = test_data_path().join("repos/valid/qa-primary");

    // none
    cmd("pkgcruft scan")
        .arg(&repo)
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();

    // single
    cmd("pkgcruft scan")
        .arg(&repo)
        .args(["--exit", "DependencyDeprecated"])
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .failure()
        .code(1);

    // multiple
    cmd("pkgcruft scan")
        .arg(&repo)
        .args(["--exit", "DependencyDeprecated,EapiBanned"])
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .failure()
        .code(1);

    // defaults (fail on critical or error level reports)
    cmd("pkgcruft scan -r DependencyDeprecated")
        .arg(&repo)
        .arg("--exit")
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
    cmd("pkgcruft scan")
        .arg(&repo)
        .arg("--exit")
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .failure()
        .code(1);
}

#[test]
fn filters() {
    let gentoo_repo_path = test_data_path().join("repos/valid/gentoo");
    let primary_repo_path = test_data_path().join("repos/valid/qa-primary");
    let expected = glob_reports!("{gentoo_repo_path}/Header/HeaderInvalid/reports.json");

    // none
    let reports = cmd("pkgcruft scan -R json")
        .args(["-r", "HeaderInvalid"])
        .arg(&gentoo_repo_path)
        .to_reports()
        .unwrap();
    assert_unordered_eq!(&reports, &expected);

    for opt in ["-f", "--filters"] {
        // invalid
        cmd("pkgcruft scan -R json")
            .args([opt, "invalid"])
            .args(["-r", "HeaderInvalid"])
            .arg(&gentoo_repo_path)
            .assert()
            .stdout("")
            .stderr(contains("--filter"))
            .failure()
            .code(2);

        // invalid custom
        cmd("pkgcruft scan -R json")
            .args([opt, "slot = 1"])
            .args(["-r", "HeaderInvalid"])
            .arg(&gentoo_repo_path)
            .assert()
            .stdout("")
            .stderr(contains("--filter"))
            .failure()
            .code(2);

        // valid and invalid
        cmd("pkgcruft scan -R json")
            .args([opt, "latest"])
            .args([opt, "invalid"])
            .args(["-r", "HeaderInvalid"])
            .arg(&gentoo_repo_path)
            .assert()
            .stdout("")
            .stderr(contains("--filter"))
            .failure()
            .code(2);

        // latest
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "latest"])
            .args(["-r", "HeaderInvalid"])
            .arg(&gentoo_repo_path)
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&reports, &expected[4..]);

        // latest inverted
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "!latest"])
            .args(["-r", "HeaderInvalid"])
            .arg(&gentoo_repo_path)
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&reports, &expected[..4]);

        // chaining a filter and its inversion returns no results
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "latest"])
            .args([opt, "!latest"])
            .args(["-r", "HeaderInvalid"])
            .arg(&gentoo_repo_path)
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&reports, &[]);

        // latest slots
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "latest-slots"])
            .args(["-r", "HeaderInvalid"])
            .arg(&gentoo_repo_path)
            .to_reports()
            .unwrap();
        assert_unordered_eq!(reports, [&expected[1..=1], &expected[4..]].concat());

        // latest slots inverted
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "!latest-slots"])
            .args(["-r", "HeaderInvalid"])
            .arg(&gentoo_repo_path)
            .to_reports()
            .unwrap();
        assert_unordered_eq!(reports, [&expected[..1], &expected[2..4]].concat());

        // live
        let live_reports = glob_reports!("{primary_repo_path}/Keywords/KeywordsLive/reports.json");
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "live"])
            .args(["-r", "KeywordsLive"])
            .arg(&primary_repo_path)
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&reports, &live_reports);

        // live inverted
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "!live"])
            .args(["-r", "KeywordsLive"])
            .arg(primary_repo_path.join("Keywords/KeywordsLive"))
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&reports, &[]);

        // stable
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "stable"])
            .args(["-r", "HeaderInvalid"])
            .arg(&gentoo_repo_path)
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&reports, &expected[..3]);

        // unstable
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "!stable"])
            .args(["-r", "HeaderInvalid"])
            .arg(&gentoo_repo_path)
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&reports, &expected[3..]);

        // stable + latest
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "stable"])
            .args([opt, "latest"])
            .args(["-r", "HeaderInvalid"])
            .arg(&gentoo_repo_path)
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&reports, &expected[2..=2]);

        // masked
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "masked"])
            .args(["-r", "HeaderInvalid"])
            .arg(&gentoo_repo_path)
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&reports, &expected[..1]);

        // unmasked
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "!masked"])
            .args(["-r", "HeaderInvalid"])
            .arg(&gentoo_repo_path)
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&reports, &expected[1..]);

        // custom
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "slot == '1'"])
            .args(["-r", "HeaderInvalid"])
            .arg(&gentoo_repo_path)
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&reports, &expected[2..]);

        // custom inverted
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "!slot == '1'"])
            .args(["-r", "HeaderInvalid"])
            .arg(&gentoo_repo_path)
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&reports, &expected[..2]);
    }
}

#[test]
fn reporter() {
    let repo = test_data_path().join("repos/valid/qa-primary");
    env::set_current_dir(&repo).unwrap();

    for opt in ["-R", "--reporter"] {
        // invalid
        cmd("pkgcruft scan")
            .args([opt, "invalid"])
            .assert()
            .stdout("")
            .stderr(contains("--reporter"))
            .failure()
            .code(2);

        // valid
        for reporter in ["simple", "fancy", "json"] {
            cmd("pkgcruft scan")
                .args([opt, reporter])
                .assert()
                .stdout(predicate::str::is_empty().not())
                .stderr("")
                .success();
        }

        // missing format string
        cmd("pkgcruft scan")
            .args([opt, "format"])
            .assert()
            .stdout("")
            .stderr(contains("--format"))
            .failure()
            .code(2);

        // invalid format string
        cmd("pkgcruft scan")
            .args([opt, "format"])
            .args(["--format", "{format}"])
            .assert()
            .stdout("")
            .stderr(contains("invalid output format"))
            .failure();

        // valid format string
        cmd("pkgcruft scan")
            .args([opt, "format"])
            .args(["--format", "{package}"])
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();
    }
}

#[test]
fn checks() {
    let repo = test_data_path().join("repos/valid/qa-primary");
    let single_expected = glob_reports!("{repo}/Dependency/**/reports.json");
    let multiple_expected = glob_reports!(
        "{repo}/Dependency/**/reports.json",
        "{repo}/EapiStatus/**/reports.json",
        "{repo}/Keywords/**/reports.json",
    );

    for opt in ["-c", "--checks"] {
        // invalid
        cmd("pkgcruft scan -R json")
            .args([opt, "invalid"])
            .arg(&repo)
            .assert()
            .stdout("")
            .stderr(contains("--checks"))
            .failure()
            .code(2);

        // single
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "Dependency"])
            .arg(&repo)
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&single_expected, &reports);

        // multiple
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "Dependency,EapiStatus,Keywords"])
            .arg(&repo)
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&multiple_expected, &reports);
    }
}

#[test]
fn levels() {
    let repo = test_data_path().join("repos/valid/qa-primary");
    let single_expected = glob_reports!("{repo}/EapiStatus/EapiDeprecated/reports.json");
    let multiple_expected = glob_reports!("{repo}/EapiStatus/**/reports.json");

    for opt in ["-l", "--levels"] {
        // invalid
        cmd("pkgcruft scan -R json")
            .args([opt, "invalid"])
            .arg(&repo)
            .assert()
            .stdout("")
            .stderr(contains("--levels"))
            .failure()
            .code(2);

        // single
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "warning"])
            .arg(repo.join("EapiStatus"))
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&single_expected, &reports);

        // multiple
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "warning,error"])
            .arg(repo.join("EapiStatus"))
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&multiple_expected, &reports);
    }
}

#[test]
fn reports() {
    let repo = test_data_path().join("repos/valid/qa-primary");
    let single_expected = glob_reports!("{repo}/Dependency/DependencyDeprecated/reports.json");
    let multiple_expected = glob_reports!(
        "{repo}/Dependency/DependencyDeprecated/reports.json",
        "{repo}/EapiStatus/EapiBanned/reports.json",
        "{repo}/Keywords/KeywordsUnsorted/reports.json",
    );

    for opt in ["-r", "--reports"] {
        // invalid
        cmd("pkgcruft scan -R json")
            .args([opt, "invalid"])
            .arg(&repo)
            .assert()
            .stdout("")
            .stderr(contains("--reports"))
            .failure()
            .code(2);

        // single
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "DependencyDeprecated"])
            .arg(&repo)
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&single_expected, &reports);

        // multiple
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "DependencyDeprecated,EapiBanned,KeywordsUnsorted"])
            .arg(&repo)
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&multiple_expected, &reports);
    }
}

#[test]
fn scopes() {
    let repo = test_data_path().join("repos/valid/qa-primary");
    let single_expected = glob_reports!("{repo}/Dependency/DependencyDeprecated/reports.json");
    let multiple_expected = glob_reports!(
        "{repo}/Dependency/DependencyDeprecated/reports.json",
        "{repo}/UseLocal/UseLocal/reports.json",
    );

    for opt in ["-s", "--scopes"] {
        // invalid
        cmd("pkgcruft scan -R json")
            .args([opt, "invalid"])
            .arg(&repo)
            .assert()
            .stdout("")
            .stderr(contains("--scopes"))
            .failure()
            .code(2);

        // single
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "version"])
            .arg(repo.join("Dependency/DependencyDeprecated"))
            .arg(repo.join("UseLocal/UseLocal"))
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&single_expected, &reports);

        // multiple
        let reports = cmd("pkgcruft scan -R json")
            .args([opt, "version,package"])
            .arg(repo.join("Dependency/DependencyDeprecated"))
            .arg(repo.join("UseLocal/UseLocal"))
            .to_reports()
            .unwrap();
        assert_unordered_eq!(&multiple_expected, &reports);
    }
}
