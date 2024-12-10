use std::fs::File;
use std::io::Write;
use std::process::Command;

use camino::{Utf8DirEntry, Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use scallop::{Error, ExecStatus};

use crate::io::stdout;

use super::make_builtin;

const LONG_DOC: &str = "Apply patches to a package's source code.";

/// Try to apply a path as a patch.
fn apply_patch(path: &Utf8Path, options: &[&str]) -> scallop::Result<()> {
    let data = File::open(path).map_err(|e| Error::Base(format!("invalid patch: {path}: {e}")))?;

    let patch = Command::new("patch")
        .args(["-p1", "-f", "-g0", "--no-backup-if-mismatch"])
        .args(options)
        .stdin(data)
        .output()
        .map_err(|e| Error::Base(format!("patch failed: {e}")))?;

    if patch.status.success() {
        Ok(())
    } else {
        let error = String::from_utf8_lossy(&patch.stdout);
        Err(Error::Base(format!("failed applying: {path}\n{error}")))
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

struct FindPatches<'a>(std::vec::IntoIter<&'a Utf8Path>);

/// Return all the patches for a given path.
fn patches_from_path(path: &Utf8Path) -> scallop::Result<(Option<&Utf8Path>, Vec<Utf8PathBuf>)> {
    if path.is_dir() {
        let mut dir_patches: Vec<_> = path
            .read_dir_utf8()?
            .filter_map(|e| match e {
                Ok(e) if is_patch(&e) => Some(Ok(e.into_path())),
                Ok(_) => None,
                Err(e) => Some(Err(Error::Base(format!("failed reading patches: {path}: {e}")))),
            })
            .try_collect()?;

        // this sorts by utf8 not the POSIX locale specified by PMS
        dir_patches.sort();

        if dir_patches.is_empty() {
            Err(Error::Base(format!("no patches in directory: {path}")))
        } else {
            Ok((Some(path), dir_patches))
        }
    } else {
        Ok((None, vec![path.to_path_buf()]))
    }
}

impl<'a> Iterator for FindPatches<'a> {
    type Item = scallop::Result<(Option<&'a Utf8Path>, Vec<Utf8PathBuf>)>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(patches_from_path)
    }
}

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    // split args into options and files
    let (mut files, mut options) = (vec![], vec![]);
    let mut args = args.iter().copied();
    for arg in &mut args {
        if arg.starts_with('-') {
            if !files.is_empty() {
                return Err(Error::Base("options must be specified before file arguments".into()));
            } else if arg == "--" {
                files.extend(args.map(Utf8Path::new));
                break;
            } else {
                options.push(arg);
            }
        } else {
            files.push(Utf8Path::new(arg));
        }
    }

    if files.is_empty() {
        return Err(Error::Base("no patches specified".to_string()));
    }

    let mut stdout = stdout();
    for patches in FindPatches(files.into_iter()) {
        let (dir, paths) = patches?;
        if let Some(path) = &dir {
            writeln!(stdout, "Applying patches from {path}")?;
        }

        for path in paths {
            if dir.is_some() {
                writeln!(stdout, "  {path}...")?;
            } else {
                writeln!(stdout, "Applying {path}...")?;
            }
            apply_patch(&path, &options)?;
        }
    }

    Ok(ExecStatus::Success)
}

const USAGE: &str = "eapply file.patch";
make_builtin!("eapply", eapply_builtin);

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use tempfile::tempdir;

    use crate::test::assert_err_re;

    use super::super::{assert_invalid_args, cmd_scope_tests, eapply};
    use super::*;

    cmd_scope_tests!(USAGE);

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
            let path = args.first().unwrap();
            assert_err_re!(r, format!("^invalid patch: {path}: No such file or directory"));
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
        let file_content = indoc::indoc! {"
            1
        "};
        let bad_content = indoc::indoc! {"
            --- a/file.txt
            +++ a/file.txt
            @@ -1 +1 @@
            -2
            +3
        "};
        let bad_prefix = indoc::indoc! {"
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
        let file_content = indoc::indoc! {"
            0
        "};
        let good_content = indoc::indoc! {"
            --- a/file.txt
            +++ a/file.txt
            @@ -1 +1 @@
            -0
            +1
        "};
        let different_prefix = indoc::indoc! {"
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
        let file_content = indoc::indoc! {"
            0
        "};
        let patch0 = indoc::indoc! {"
            --- a/file.txt
            +++ a/file.txt
            @@ -1 +1 @@
            -0
            +1
        "};
        let patch1 = indoc::indoc! {"
            --- a/file.txt
            +++ a/file.txt
            @@ -1 +1 @@
            -1
            +2
        "};

        let dir = tempdir().unwrap();
        env::set_current_dir(&dir).unwrap();
        fs::write("file.txt", file_content).unwrap();
        fs::create_dir_all("files/empty").unwrap();
        for (i, data) in [patch0, patch1].iter().enumerate() {
            let file = format!("files/{i}.patch");
            fs::write(file, data).unwrap();
        }

        // verify patch searching isn't recursive
        fs::create_dir_all("files/nested").unwrap();
        fs::write("files/nested/nested.patch", patch1).unwrap();

        // apply patches from target directory and verify content
        assert_eq!(fs::read_to_string("file.txt").unwrap(), "0\n");
        eapply(&["files"]).unwrap();
        assert_eq!(fs::read_to_string("file.txt").unwrap(), "2\n");
    }
}
