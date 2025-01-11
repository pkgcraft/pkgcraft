use std::env;

use itertools::Itertools;
use pkgcraft::test::{cmd, test_data};
use pkgcruft::check::Check;
use predicates::prelude::*;
use strum::IntoEnumIterator;

#[test]
fn all() {
    cmd("pkgcruft show checks")
        .assert()
        .stdout(indoc::formatdoc! {"
            {}
        ", Check::iter().join("\n")})
        .stderr("")
        .success();
}

#[test]
fn info() {
    for opt in ["-i", "--info"] {
        cmd("pkgcruft show checks")
            .arg(opt)
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();
    }
}

#[test]
fn repo() {
    let data = test_data();
    let repo = data.ebuild_repo("qa-primary").unwrap();

    cmd("pkgcruft show checks --repo")
        .arg(repo)
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();

    // current working directory
    env::set_current_dir(repo).unwrap();
    cmd("pkgcruft show checks --repo")
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}
