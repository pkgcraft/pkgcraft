use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::{env, fs};

use tempfile::{tempdir, TempDir};
use walkdir::WalkDir;

use crate::pkgsh::BUILD_DATA;

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

    pub(crate) fn files(&self) -> Vec<String> {
        let mut files = Vec::<String>::new();
        let root = Path::new("/");
        for entry in WalkDir::new(&self.install_dir) {
            let entry = entry.unwrap();
            match entry.path() {
                p if p.is_dir() => continue,
                p => {
                    let new_path = root.join(p.strip_prefix(&self.install_dir).unwrap());
                    files.push(new_path.to_str().unwrap().into());
                }
            }
        }
        files
    }

    pub(crate) fn modes(&self) -> Vec<(String, u32)> {
        let mut modes = Vec::<(String, u32)>::new();
        for file in self.files() {
            let file_path = file.strip_prefix("/").unwrap();
            let meta = fs::metadata(self.install_dir.join(file_path)).unwrap();
            modes.push((file, meta.mode()));
        }
        modes
    }

    pub(crate) fn wipe(&self) {
        fs::remove_dir_all(&self.install_dir).unwrap();
    }

    pub(crate) fn run<F: FnOnce()>(&self, func: F) {
        func();
        self.wipe();
    }
}
