#![cfg(test)]

use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::{env, fs};

use camino::Utf8PathBuf;
use camino_tempfile::Utf8TempDir;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::shell::environment::Variable::ED;

#[derive(Debug, Deserialize, Serialize)]
struct Files {
    files: Vec<FileData>,
}

#[derive(Debug, Deserialize, Serialize)]
struct FileData {
    pub(crate) path: String,
    mode: Option<u32>,
    data: Option<String>,
    link: Option<String>,
}

#[derive(Debug)]
pub(crate) struct FileTree {
    _tmpdir: Utf8TempDir,
    pub(crate) install_dir: Utf8PathBuf,
}

impl FileTree {
    pub(crate) fn new() -> Self {
        let tmpdir = Utf8TempDir::new().unwrap();
        let src_dir = tmpdir.path().join("src");
        let install_dir = tmpdir.path().join("image");

        crate::shell::get_build_mut()
            .env
            .insert(ED, install_dir.to_string());

        fs::create_dir(&install_dir).unwrap();
        fs::create_dir(&src_dir).unwrap();
        env::set_current_dir(&src_dir).unwrap();
        FileTree { _tmpdir: tmpdir, install_dir }
    }

    pub(crate) fn wipe(&self) {
        fs::remove_dir_all(&self.install_dir).unwrap();
        fs::create_dir(&self.install_dir).unwrap();
    }

    pub(crate) fn assert<S: AsRef<str>>(&self, data: S) {
        // load expected data from toml
        let data: Files = toml::from_str(data.as_ref()).unwrap();
        let mut files = data.files;
        files.reverse();

        // match expected data against fs data
        let root = Path::new("/");
        for entry in WalkDir::new(&self.install_dir)
            .min_depth(1)
            .sort_by_file_name()
        {
            let entry = entry.unwrap();
            let path = entry.path();

            // skip non-empty subdirs
            if path.is_dir() && path.read_dir().unwrap().next().is_some() {
                continue;
            }

            let file_path = root.join(path.strip_prefix(&self.install_dir).unwrap());
            let meta = fs::symlink_metadata(path).unwrap();
            let expected = files
                .pop()
                .unwrap_or_else(|| panic!("unknown path: {}", path.display()));
            assert_eq!(file_path.to_string_lossy(), expected.path);

            if let Some(expected) = expected.mode {
                let file_mode = meta.mode();
                assert!(
                    file_mode == expected,
                    "{file_path:?}: mode {file_mode:#o} is not {expected:#o}"
                );
            }

            if let Some(expected) = &expected.data {
                let file_data = fs::read_to_string(path).unwrap();
                assert_eq!(file_data, expected.as_str());
            }

            if let Some(expected) = expected.link.as_deref() {
                let target = path.read_link().unwrap();
                assert_eq!(target.to_string_lossy(), expected);
            }
        }

        assert!(files.is_empty(), "unmatched files: {files:?}");

        self.wipe();
    }

    pub(crate) fn is_empty(&self) -> bool {
        WalkDir::new(&self.install_dir)
            .min_depth(1)
            .into_iter()
            .next()
            .is_none()
    }
}
