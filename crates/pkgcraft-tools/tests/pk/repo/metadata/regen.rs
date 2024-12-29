use std::{env, fs};

use indexmap::IndexMap;
use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::cache::Cache;
use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::{assert_unordered_eq, cmd, test_data};
use predicates::prelude::*;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use walkdir::WalkDir;

use crate::predicates::lines_contain;

#[test]
fn nonexistent_repo() {
    cmd("pk repo metadata regen path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(contains("unknown repo: path/to/nonexistent/repo"))
        .failure()
        .code(2);

    cmd("pk repo metadata regen nonexistent-repo-alias")
        .assert()
        .stdout("")
        .stderr(contains("unknown repo: nonexistent-repo-alias"))
        .failure()
        .code(2);
}

#[test]
fn empty_repo() {
    let data = test_data();
    let repo = data.ebuild_repo("empty").unwrap();
    cmd("pk repo metadata regen")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(!repo.metadata().cache().path().exists());
}

#[test]
fn default_current_directory() {
    // non-repo working directory
    let dir = tempdir().unwrap();
    env::set_current_dir(dir.path()).unwrap();
    cmd("pk repo metadata regen")
        .assert()
        .stdout("")
        .stderr(contains("non-ebuild repo: ."))
        .failure()
        .code(2);

    // repo working directory
    let data = test_data();
    let repo = data.ebuild_repo("metadata").unwrap();
    env::set_current_dir(repo).unwrap();
    cmd("pk repo metadata regen")
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn progress() {
    let data = test_data();
    let repo = data.ebuild_repo("empty").unwrap();
    for opt in ["-n", "--no-progress"] {
        cmd("pk repo metadata regen")
            .arg(opt)
            .arg(repo)
            .assert()
            .stdout("")
            .stderr("")
            .success();
    }
}

#[test]
fn single() {
    let mut config = Config::default();
    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    temp.create_ebuild("cat/pkg-1", &["EAPI=7"]).unwrap();
    let repo = config
        .add_repo(&temp, false)
        .unwrap()
        .into_ebuild()
        .unwrap();

    // default target is the current working directory
    env::set_current_dir(&repo).unwrap();
    cmd("pk repo metadata regen")
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let path = repo.metadata().cache().path().join("cat/pkg-1");
    assert!(path.exists());
    let prev_modified = fs::metadata(&path).unwrap().modified().unwrap();

    // re-run doesn't change cache
    cmd("pk repo metadata regen")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    assert_eq!(modified, prev_modified);
    let prev_modified = modified;

    // package changes cause cache updates
    temp.create_ebuild("cat/pkg-1", &["EAPI=8"]).unwrap();
    cmd("pk repo metadata regen")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    assert_ne!(modified, prev_modified);
    let prev_modified = modified;

    // -f/--force option cause cache updates
    for opt in ["-f", "--force"] {
        cmd("pk repo metadata regen")
            .arg(opt)
            .arg(&repo)
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
    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    temp.create_ebuild("cat/pkg-1", &[]).unwrap();

    for opt in ["-j", "--jobs"] {
        // invalid
        for val in ["", "-1"] {
            cmd("pk repo metadata regen")
                .args([opt, val])
                .assert()
                .stdout("")
                .stderr(predicate::str::is_empty().not())
                .failure()
                .code(2);
        }

        // valid and automatically bounded between 1 and max CPUs
        for val in ["0", "999999"] {
            cmd("pk repo metadata regen")
                .args([opt, val])
                .arg(temp.path())
                .assert()
                .stdout("")
                .stderr("")
                .success();
        }
    }
}

#[test]
fn multiple() {
    let mut config = Config::default();
    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    temp.create_ebuild("cat/a-1", &[]).unwrap();
    temp.create_ebuild("cat/b-1", &[]).unwrap();
    temp.create_ebuild("other/pkg-1", &[]).unwrap();
    let repo = config
        .add_repo(&temp, false)
        .unwrap()
        .into_ebuild()
        .unwrap();

    cmd("pk repo metadata regen")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();

    let path = repo.metadata().cache().path();
    assert!(path.join("cat/a-1").exists());
    assert!(path.join("cat/b-1").exists());
    assert!(path.join("other").exists());

    // outdated cache files and directories are removed
    fs::remove_dir_all(repo.path().join("cat/b")).unwrap();
    fs::remove_dir_all(repo.path().join("other")).unwrap();
    cmd("pk repo metadata regen")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(path.join("cat/a-1").exists());
    assert!(!path.join("cat/b-1").exists());
    assert!(!path.join("other").exists());
}

#[test]
fn pkg_with_invalid_eapi() {
    let mut config = Config::default();
    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    temp.create_ebuild("cat/a-1", &["EAPI=invalid"]).ok();
    temp.create_ebuild("cat/b-1", &["EAPI=8"]).unwrap();
    let repo = config
        .add_repo(&temp, false)
        .unwrap()
        .into_ebuild()
        .unwrap();

    cmd("pk repo metadata regen")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr(lines_contain(["invalid pkg: cat/a-1", "metadata failures occurred"]))
        .failure()
        .code(2);

    let path = repo.metadata().cache().path();
    assert!(!path.join("cat/a-1").exists());
    assert!(path.join("cat/b-1").exists());
}

#[test]
fn pkg_with_invalid_dep() {
    let mut config = Config::default();
    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    temp.create_ebuild("cat/a-1", &["DEPEND=cat/pkg[]"]).ok();
    temp.create_ebuild("cat/b-1", &["DEPEND=cat/pkg"]).unwrap();
    let repo = config
        .add_repo(&temp, false)
        .unwrap()
        .into_ebuild()
        .unwrap();

    cmd("pk repo metadata regen")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr(lines_contain(["invalid pkg: cat/a-1", "metadata failures occurred"]))
        .failure()
        .code(2);

    let path = repo.metadata().cache().path();
    assert!(!path.join("cat/a-1").exists());
    assert!(path.join("cat/b-1").exists());
}

#[test]
fn data_content() {
    let data = test_data();
    let repo = data.ebuild_repo("metadata").unwrap();

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
    let expected: IndexMap<_, _> = metadata_content(repo.metadata().cache().path().as_str());

    // regenerate metadata
    for opt in ["-p", "--path"] {
        let dir = tempdir().unwrap();
        let cache_path = dir.path().to_str().unwrap();

        cmd("pk repo metadata regen")
            .args([opt, cache_path])
            .arg(repo)
            .assert()
            .stdout("")
            .stderr("")
            .success();

        // verify new data matches original
        let new: IndexMap<_, _> = metadata_content(cache_path);
        for (cpv, data) in new {
            assert_unordered_eq!(expected.get(&cpv).unwrap().lines(), data.lines());
        }
    }
}
