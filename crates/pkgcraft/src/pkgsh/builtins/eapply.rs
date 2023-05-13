use std::fs::File;
use std::process::Command;

use camino::{Utf8DirEntry, Utf8Path, Utf8PathBuf};
use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::pkgsh::write_stdout;

use super::make_builtin;

const LONG_DOC: &str = "Apply patches to a package's source code.";

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
struct PatchFile {
    path: Utf8PathBuf,
    name: String,
}

impl PatchFile {
    fn new(path: Utf8PathBuf) -> scallop::Result<Self> {
        match path.file_name() {
            Some(name) => {
                let name = name.to_string();
                Ok(Self { path, name })
            }
            None => Err(Error::Base(format!("invalid patch file: {path}"))),
        }
    }

    fn apply(&self, options: &[&str]) -> scallop::Result<()> {
        let path = &self.path;
        let data = File::open(path)
            .map_err(|e| Error::Base(format!("failed reading patch: {path}: {e}")))?;

        let output = Command::new("patch")
            .args(["-p1", "-f", "-g0", "--no-backup-if-mismatch"])
            .args(options)
            .stdin(data)
            .output()
            .map_err(|e| Error::Base(format!("failed running patch: {e}")))?;

        if output.status.success() {
            Ok(())
        } else {
            let error = String::from_utf8_lossy(&output.stdout);
            Err(Error::Base(format!("failed applying: {path}\n{error}")))
        }
    }
}

// Predicate used to filter compatible patch files from an iterator.
fn is_patch(entry: &Utf8DirEntry) -> bool {
    let path = entry.path();
    if path.is_dir() {
        false
    } else {
        path.extension()
            .map(|s| s == "diff" || s == "patch")
            .unwrap_or(false)
    }
}

type Patches = Vec<(Option<Utf8PathBuf>, Vec<PatchFile>)>;

// Find the patches contained in a given set of paths.
fn find_patches<P: AsRef<Utf8Path>>(paths: &[P]) -> scallop::Result<Patches> {
    let mut patches = Patches::new();
    for path in paths.iter().map(|p| p.as_ref()) {
        if path.is_dir() {
            let dir_patches: scallop::Result<Vec<_>> = path
                .read_dir_utf8()?
                .filter_map(|e| match e {
                    Ok(e) if is_patch(&e) => Some(PatchFile::new(e.into_path())),
                    Ok(_) => None,
                    Err(e) => {
                        Some(Err(Error::Base(format!("failed reading patches: {path}: {e}"))))
                    }
                })
                .collect();

            let mut dir_patches = dir_patches?;

            if dir_patches.is_empty() {
                return Err(Error::Base(format!("no patches in directory: {path}")));
            }

            // note that this currently sorts by utf8 not the POSIX locale specified by PMS
            dir_patches.sort();
            patches.push((Some(path.into()), dir_patches));
        } else if path.exists() {
            patches.push((None, vec![PatchFile::new(path.to_path_buf())?]));
        } else {
            return Err(Error::Base(format!("nonexistent file: {path}")));
        }
    }

    Ok(patches)
}

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let mut options = Vec::<&str>::new();
    let mut files = Vec::<&str>::new();
    for (i, arg) in args.iter().enumerate() {
        if arg.starts_with('-') {
            if !files.is_empty() {
                return Err(Error::Base("options must be specified before file arguments".into()));
            }
            if *arg == "--" {
                files.extend(&args[i + 1..]);
                break;
            } else {
                options.push(arg);
            }
        } else {
            files.push(arg);
        }
    }

    if files.is_empty() {
        return Err(Error::Base("no patches specified".to_string()));
    }

    for (dir, files) in find_patches(&files)? {
        let msg_prefix = match &dir {
            None => "",
            Some(path) => {
                write_stdout!("Applying patches from {path}\n")?;
                "  "
            }
        };

        for f in files {
            match &dir {
                None => write_stdout!("{msg_prefix}Applying {}...\n", f.name)?,
                _ => write_stdout!("{msg_prefix}{}...\n", f.name)?,
            }
            f.apply(&options)?;
        }
    }

    Ok(ExecStatus::Success)
}

const USAGE: &str = "eapply file.patch";
make_builtin!("eapply", eapply_builtin, run, LONG_DOC, USAGE, &[("6..", &["src_prepare"])]);

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;

    use indoc::indoc;
    use tempfile::tempdir;

    use crate::macros::assert_err_re;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as eapply;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(eapply, &[0]);

        // options after file args
        for args in [&["file.patch", "--"], &["file.patch", "-p1"]] {
            let r = eapply(args);
            assert_err_re!(r, "^options must be specified before file arguments$");
        }

        // no file args
        for args in [&["--"], &["-p1"]] {
            let r = eapply(args);
            assert_err_re!(r, "^no patches specified$");
        }

        // nonexistent files
        for args in [vec!["file.patch"], vec!["--", "--"]] {
            let r = eapply(&args);
            assert_err_re!(r, "^nonexistent file: .*$");
        }

        // empty dir
        let dir = tempdir().unwrap();
        env::set_current_dir(&dir).unwrap();
        fs::create_dir("files").unwrap();
        let r = eapply(&["files"]);
        assert_err_re!(r, "^no patches in directory: .*$");
    }

    #[test]
    fn patch_failures() {
        let file_content: &str = indoc! {"
            1
        "};
        let bad_content: &str = indoc! {"
            --- a/file.txt
            +++ a/file.txt
            @@ -1 +1 @@
            -2
            +3
        "};
        let bad_prefix: &str = indoc! {"
            --- a/b/file.txt
            +++ a/b/file.txt
            @@ -1 +1 @@
            -1
            +2
        "};

        let dir = tempdir().unwrap();
        env::set_current_dir(&dir).unwrap();
        fs::write("file.txt", file_content).unwrap();
        for data in [bad_content, bad_prefix] {
            fs::write("file.patch", data).unwrap();
            let r = eapply(&["file.patch"]);
            assert_err_re!(r, "^failed applying: file.patch");
        }
    }

    #[test]
    fn file_patch() {
        let file_content: &str = indoc! {"
            0
        "};
        let good_content: &str = indoc! {"
            --- a/file.txt
            +++ a/file.txt
            @@ -1 +1 @@
            -0
            +1
        "};
        let different_prefix: &str = indoc! {"
            --- a/b/file.txt
            +++ a/b/file.txt
            @@ -1 +1 @@
            -1
            +2
        "};

        let dir = tempdir().unwrap();
        env::set_current_dir(&dir).unwrap();
        fs::write("file.txt", file_content).unwrap();
        assert_eq!(fs::read_to_string("file.txt").unwrap(), "0\n");
        for (opts, data) in [(vec![], good_content), (vec!["-p2"], different_prefix)] {
            fs::write("file.patch", data).unwrap();
            let args = [opts, vec!["file.patch"]].concat();
            eapply(&args).unwrap();
        }
        assert_eq!(fs::read_to_string("file.txt").unwrap(), "2\n");
    }

    #[test]
    fn dir_patches() {
        let file_content: &str = indoc! {"
            0
        "};
        let patch0: &str = indoc! {"
            --- a/file.txt
            +++ a/file.txt
            @@ -1 +1 @@
            -0
            +1
        "};
        let patch1: &str = indoc! {"
            --- a/file.txt
            +++ a/file.txt
            @@ -1 +1 @@
            -1
            +2
        "};

        let dir = tempdir().unwrap();
        env::set_current_dir(&dir).unwrap();
        fs::write("file.txt", file_content).unwrap();
        fs::create_dir("files").unwrap();
        for (i, data) in [patch0, patch1].iter().enumerate() {
            let file = format!("files/{i}.patch");
            fs::write(file, data).unwrap();
        }
        assert_eq!(fs::read_to_string("file.txt").unwrap(), "0\n");
        eapply(&["files"]).unwrap();
        assert_eq!(fs::read_to_string("file.txt").unwrap(), "2\n");
    }
}
