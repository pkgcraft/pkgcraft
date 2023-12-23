use std::{env, fs};

use indexmap::IndexMap;
use pkgcraft::repo::{ebuild::temp::Repo as TempRepo, Repository};
use pkgcraft::test::{assert_unordered_eq, cmd, TEST_DATA};
use predicates::prelude::*;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use walkdir::WalkDir;

use crate::predicates::lines_contain;

#[test]
fn missing_repo_arg() {
    cmd("pk repo metadata")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn nonexistent_repo() {
    cmd("pk repo metadata")
        .arg("path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn no_pkgs() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    cmd("pk repo metadata")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(!t.repo().metadata().cache_path().exists());
}

#[test]
fn single() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_raw_pkg("cat/pkg-1", &["EAPI=7"]).unwrap();

    // default target is the current working directory
    env::set_current_dir(t.path()).unwrap();
    cmd("pk repo metadata")
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let path = t.repo().metadata().cache_path().join("cat/pkg-1");
    assert!(path.exists());
    let prev_modified = fs::metadata(&path).unwrap().modified().unwrap();

    // re-run doesn't change cache
    cmd("pk repo metadata")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    assert_eq!(modified, prev_modified);
    let prev_modified = modified;

    // package changes cause cache updates
    t.create_raw_pkg("cat/pkg-1", &["EAPI=8"]).unwrap();
    cmd("pk repo metadata")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    assert_ne!(modified, prev_modified);
    let prev_modified = modified;

    // -f/--force option cause cache updates
    for opt in ["-f", "--force"] {
        cmd("pk repo metadata")
            .arg(opt)
            .arg(t.path())
            .assert()
            .stdout("")
            .stderr("")
            .success();

        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        assert_ne!(modified, prev_modified);
    }
}

#[test]
fn jobs() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_raw_pkg("cat/pkg-1", &[]).unwrap();

    for opt in ["-j", "--jobs"] {
        // invalid
        for val in ["", "-1"] {
            cmd("pk repo metadata")
                .args([opt, val])
                .assert()
                .stdout("")
                .stderr(predicate::str::is_empty().not())
                .failure()
                .code(2);
        }

        // valid and automatically bounded between 1 and max CPUs
        for val in [0, 999999] {
            cmd("pk repo metadata")
                .arg(opt)
                .arg(val.to_string())
                .arg(t.path())
                .assert()
                .stdout("")
                .stderr("")
                .success();
        }
    }
}

#[test]
fn multiple() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_raw_pkg("cat/a-1", &["EAPI=7"]).unwrap();
    t.create_raw_pkg("cat/b-1", &["EAPI=8"]).unwrap();
    cmd("pk repo metadata")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();

    let path = t.repo().metadata().cache_path();
    assert!(path.join("cat/a-1").exists());
    assert!(path.join("cat/b-1").exists());

    // outdated cache entries lacking ebuilds are removed
    fs::remove_dir_all(t.repo().path().join("cat/b")).unwrap();
    cmd("pk repo metadata")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(path.join("cat/a-1").exists());
    assert!(!path.join("cat/b-1").exists());
}

#[test]
fn pkg_with_invalid_eapi() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_raw_pkg("cat/a-1", &["EAPI=invalid"]).ok();
    t.create_raw_pkg("cat/b-1", &["EAPI=8"]).unwrap();
    cmd("pk repo metadata")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr(lines_contain(["invalid pkg: cat/a-1", "failed generating metadata"]))
        .failure()
        .code(2);

    let path = t.repo().metadata().cache_path();
    assert!(!path.join("cat/a-1").exists());
    assert!(path.join("cat/b-1").exists());
}

#[test]
fn pkg_with_invalid_dep() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_raw_pkg("cat/a-1", &["DEPEND=cat/pkg[]"]).ok();
    t.create_raw_pkg("cat/b-1", &["DEPEND=cat/pkg"]).unwrap();
    cmd("pk repo metadata")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr(lines_contain(["invalid pkg: cat/a-1", "failed generating metadata"]))
        .failure()
        .code(2);

    let path = t.repo().metadata().cache_path();
    assert!(!path.join("cat/a-1").exists());
    assert!(path.join("cat/b-1").exists());
}

#[test]
fn multiple_repos() {
    let t1 = TempRepo::new("test1", None, 0, None).unwrap();
    t1.create_raw_pkg("cat/a-1", &["EAPI=7"]).unwrap();
    let t2 = TempRepo::new("test2", None, 0, None).unwrap();
    t2.create_raw_pkg("cat/b-1", &["EAPI=8"]).unwrap();
    cmd("pk repo metadata")
        .args([t1.path(), t2.path()])
        .assert()
        .stdout("")
        .stderr("")
        .success();

    let cache_path_t1 = t1.repo().metadata().cache_path();
    assert!(cache_path_t1.join("cat/a-1").exists());
    let cache_path_t2 = t2.repo().metadata().cache_path();
    assert!(cache_path_t2.join("cat/b-1").exists());
}

#[test]
fn data_content() {
    let repo = TEST_DATA.ebuild_repo("metadata").unwrap();

    // determine metadata file content
    let metadata_content = |cache_path: &str| {
        WalkDir::new(cache_path)
            .sort_by_file_name()
            .min_depth(2)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
            .map(|e| {
                let short_path = e.path().strip_prefix(cache_path).unwrap();
                let data = fs::read_to_string(e.path()).unwrap();
                (short_path.to_path_buf(), data)
            })
            .collect()
    };

    // record expected metadata file content
    let expected: IndexMap<_, _> = metadata_content(repo.metadata().cache_path().as_str());

    // regenerate metadata
    for opt in ["-p", "--path"] {
        let dir = tempdir().unwrap();
        let cache_path = dir.path().to_str().unwrap();

        cmd("pk repo metadata")
            .args([opt, cache_path])
            .arg(repo.path())
            .assert()
            .stdout("")
            .stderr("")
            .success();

        // verify new data matches original
        let new: IndexMap<_, _> = metadata_content(cache_path);
        for (cpv, data) in new {
            assert_unordered_eq(expected.get(&cpv).unwrap().lines(), data.lines());
        }
    }
}
