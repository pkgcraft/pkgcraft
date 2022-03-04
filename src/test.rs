use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use walkdir::WalkDir;

pub(crate) static VER_CMP_DATA: &[&str] = &[
    // simple major versions
    "0 == 0",
    "0 != 1",
    // equal due to integer coercion and "-r0" being the revision default
    "0 == 0-r0",
    "1 == 01",
    "01 == 001",
    "1.00 == 1.0",
    "1.0100 == 1.010",
    "01.01 == 1.01",
    "0001.1 == 1.1",
    "1.2 == 001.2",
    "1.0.2 == 1.0.2-r0",
    "1.0.2-r0 == 1.000.2",
    "1.000.2 == 1.00.2-r0",
    "0-r0 == 0-r00",
    "0_beta01 == 0_beta001",
    "1.2_pre08-r09 == 1.2_pre8-r9",
    "1.010.02 != 1.01.2",
    // minor versions
    "0.1 < 0.11",
    "0.01 > 0.001",
    "1.0 > 1",
    "1.0_alpha > 1_alpha",
    "1.0_alpha > 1",
    "1.0_alpha < 1.0",
    // version letter suffix
    "0a < 0b",
    "1.1z > 1.1a",
    // release types
    "1_alpha < 1_beta",
    "1_beta < 1_pre",
    "1_pre < 1_rc",
    "1_rc < 1",
    "1 < 1_p",
    // release suffix vs non-suffix
    "1.2.3_alpha < 1.2.3",
    "1.2.3_beta < 1.2.3",
    "1.2.3_pre < 1.2.3",
    "1.2.3_rc < 1.2.3",
    "1.2.3_p > 1.2.3",
    // release suffix version
    "0_alpha1 < 0_alpha2",
    "0_alpha2-r1 > 0_alpha1-r2",
    "0_p1 < 0_p2",
    // last release suffix
    "0_alpha_rc_p > 0_alpha_rc",
    // revision
    "0-r2 > 0-r1",
    "1.0.2_pre01-r2 > 1.00.2_pre001-r1",
];

#[derive(Debug)]
pub(crate) struct FileTree<'a> {
    path: &'a Path,
}

impl<'a> FileTree<'a> {
    pub(crate) fn new(path: &'a Path) -> Self {
        FileTree { path: path }
    }

    pub(crate) fn files(&self) -> Vec<String> {
        let mut files = Vec::<String>::new();
        let root = Path::new("/");
        for entry in WalkDir::new(self.path) {
            let entry = entry.unwrap();
            match entry.path() {
                p if p.is_dir() => continue,
                p => {
                    let new_path = root.join(p.strip_prefix(self.path).unwrap());
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
            let meta = fs::metadata(self.path.join(file_path)).unwrap();
            modes.push((file, meta.mode()));
        }
        modes
    }
}

impl<'a> Drop for FileTree<'a> {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).unwrap();
    }
}
