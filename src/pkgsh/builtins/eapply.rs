use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use scallop::builtins::ExecStatus;
use scallop::Error;
use walkdir::{DirEntry, WalkDir};

use crate::pkgsh::write_stdout;

use super::make_builtin;

const LONG_DOC: &str = "Apply patches to a package's source code.";

type Patches = Vec<(Option<PathBuf>, Vec<PathBuf>)>;

// Predicate used to filter compatible patch files from an iterator.
fn is_patch(entry: &DirEntry) -> bool {
    let path = entry.path();
    match path.is_dir() {
        true => false,
        false => path
            .extension()
            .map(|s| s == "diff" || s == "patch")
            .unwrap_or(false),
    }
}

// Find the patches contained in a given set of paths.
fn find_patches(paths: &[&str]) -> scallop::Result<Patches> {
    let mut patches = Patches::new();
    for p in paths {
        let path = Path::new(p);
        if path.is_dir() {
            let dir_patches: Vec<PathBuf> = WalkDir::new(path)
                .sort_by_file_name()
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(is_patch)
                .map(|e| e.path().into())
                .collect();
            if dir_patches.is_empty() {
                return Err(Error::Base(format!("no patches in directory: {p:?}")));
            }
            patches.push((Some(path.into()), dir_patches));
        } else if path.exists() {
            patches.push((None, vec![path.into()]));
        } else {
            return Err(Error::Base(format!("nonexistent file: {p}")));
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
            if arg == &"--" {
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

    let patches = find_patches(&files)?;
    for (path, files) in patches.iter() {
        let msg_prefix = match path {
            None => "",
            Some(p) => {
                write_stdout!("Applying patches from {p:?}\n");
                "  "
            }
        };

        for f in files {
            let name = f.file_name().unwrap().to_string_lossy();
            match path {
                None => write_stdout!("{msg_prefix}Applying {name}...\n"),
                _ => write_stdout!("{msg_prefix}{name}...\n"),
            }
            let data = File::open(f)
                .map_err(|e| Error::Base(format!("failed reading patch {f:?}: {e}")))?;
            let output = Command::new("patch")
                .args(["-p1", "-f", "-g0", "--no-backup-if-mismatch"])
                .args(&options)
                .stdin(data)
                .output()
                .map_err(|e| Error::Base(format!("failed running patch: {e}")))?;
            if !output.status.success() {
                let error = String::from_utf8_lossy(&output.stdout);
                return Err(Error::Base(format!("failed applying: {name}\n{error}")));
            }
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
