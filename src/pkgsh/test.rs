use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::{env, fs};

use serde::{Deserialize, Serialize};
use tempfile::{tempdir, TempDir};
use walkdir::WalkDir;

use crate::pkgsh::BUILD_DATA;

#[derive(Debug, Deserialize, Serialize)]
struct Files {
    files: Vec<FileData>,
}

#[derive(Debug, Deserialize, Serialize)]
struct FileData {
    pub(crate) path: PathBuf,
    mode: Option<u32>,
    data: Option<String>,
}

#[derive(Debug)]
pub(crate) struct FileTree {
    _tmp_dir: TempDir,
    pub(crate) install_dir: PathBuf,
}

impl FileTree {
    pub(crate) fn new() -> Self {
        let tmp_dir = tempdir().unwrap();
        let path = PathBuf::from(tmp_dir.path());
        let src_dir = path.join("src");
        let install_dir = path.join("image");

        BUILD_DATA.with(|d| {
            d.borrow_mut()
                .env
                .insert("ED".into(), install_dir.to_str().unwrap().into());
        });

        fs::create_dir(&src_dir).unwrap();
        env::set_current_dir(&src_dir).unwrap();
        FileTree {
            _tmp_dir: tmp_dir,
            install_dir,
        }
    }

    pub(crate) fn wipe(&self) {
        fs::remove_dir_all(&self.install_dir).unwrap();
    }

    pub(crate) fn assert<S: AsRef<str>>(&self, data: S) {
        // load expected data from toml
        let data: Files = toml::from_str(data.as_ref()).unwrap();
        let mut files = data.files;
        files.reverse();

        // match expected data against fs data
        let root = Path::new("/");
        for entry in WalkDir::new(&self.install_dir) {
            let entry = entry.unwrap();
            match entry.path() {
                p if p.is_dir() => continue,
                p => {
                    let path = root.join(p.strip_prefix(&self.install_dir).unwrap());
                    let meta = fs::metadata(&p).unwrap();
                    let file = files.pop().unwrap_or_else(|| panic!("unknown file: {p:?}"));
                    assert_eq!(path, file.path);

                    if let Some(mode) = file.mode {
                        let path_mode = meta.mode();
                        assert!(
                            path_mode == mode,
                            "{path:?}: mode {path_mode:#o} is not expected {mode:#o}"
                        );
                    }

                    if let Some(file_data) = &file.data {
                        let path_data = fs::read_to_string(&p).unwrap();
                        assert_eq!(path_data, file_data.as_str());
                    }
                }
            }
        }

        assert!(files.is_empty(), "unmatched files: {files:?}");

        self.wipe();
    }
}
