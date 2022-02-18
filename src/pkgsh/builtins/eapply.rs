use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};
use walkdir::{DirEntry, WalkDir};

use super::PkgBuiltin;
use crate::pkgsh::write_stdout;

static LONG_DOC: &str = "Apply patches to a package's source code.";

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
fn find_patches(paths: &[&str]) -> Result<Patches> {
    let mut patches = Patches::new();
    for p in paths {
        let path = Path::new(p);
        if path.is_dir() {
            let dir_patches: Vec<PathBuf> = WalkDir::new(&path)
                .sort_by_file_name()
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(is_patch)
                .map(|e| e.path().into())
                .collect();
            if dir_patches.is_empty() {
                return Err(Error::Builtin(format!("no patches in directory: {:?}", p)));
            }
            patches.push((Some(path.into()), dir_patches));
        } else if path.exists() {
            patches.push((None, vec![path.into()]));
        } else {
            return Err(Error::Builtin(format!("nonexistent file: {}", p)));
        }
    }

    Ok(patches)
}

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    let mut options = Vec::<&str>::new();
    let mut files = Vec::<&str>::new();
    for (i, arg) in args.iter().enumerate() {
        if arg.starts_with('-') {
            if !files.is_empty() {
                return Err(Error::Builtin(
                    "options must be specified before file arguments".into(),
                ));
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
        return Err(Error::Builtin("no patches specified".to_string()));
    }

    let patches = find_patches(&files)?;
    for (path, files) in patches.iter() {
        let msg_prefix = match path {
            None => "",
            Some(p) => {
                write_stdout!("Applying patches from {:?}\n", p);
                "  "
            }
        };

        for f in files {
            let name = f.file_name().unwrap().to_string_lossy();
            match path {
                None => write_stdout!("{}Applying {}...\n", msg_prefix, name),
                _ => write_stdout!("{}{}...\n", msg_prefix, name),
            }
            let data = File::open(f)
                .map_err(|e| Error::Builtin(format!("failed reading patch {:?}: {}", f, e)))?;
            let output = Command::new("patch")
                .args(["-p1", "-f", "-g0", "--no-backup-if-mismatch"])
                .args(&options)
                .stdin(data)
                .output()
                .map_err(|e| Error::Builtin(format!("failed running patch: {}", e)))?;
            if !output.status.success() {
                let error = str::from_utf8(&output.stdout).expect("failed decoding patch output");
                return Err(Error::Builtin(format!("failed applying: {}\n{}", name, error)));
            }
        }
    }

    Ok(ExecStatus::Success)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "eapply",
            func: run,
            help: LONG_DOC,
            usage: "eapply file.patch",
        },
        &[("6-", &["src_prepare"])],
    )
});

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;

    use indoc::indoc;
    use rusty_fork::rusty_fork_test;
    use tempfile::tempdir;

    use super::super::assert_invalid_args;
    use super::run as eapply;
    use crate::macros::assert_err_re;

    rusty_fork_test! {
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
                assert_err_re!(r, format!("^nonexistent file: .*$"));
            }

            // empty dir
            let dir = tempdir().unwrap();
            env::set_current_dir(&dir).unwrap();
            fs::create_dir("files").unwrap();
            let r = eapply(&["files"]);
            assert_err_re!(r, format!("^no patches in directory: .*$"));
        }

        #[test]
        fn failure() {
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
        fn success() {
            let file_content: &str = indoc! {"
                1
            "};
            let good_content: &str = indoc! {"
                --- a/file.txt
                +++ a/file.txt
                @@ -1 +1 @@
                -1
                +2
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
            for (opts, data) in [(vec![], good_content), (vec!["-p2"], different_prefix)] {
                fs::write("file.txt", file_content).unwrap();
                fs::write("file.patch", data).unwrap();
                let args = [opts, vec!["file.patch"]].concat();
                eapply(&args).unwrap();
            }
        }

        #[test]
        fn dir() {
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
                let file = format!("files/{}.patch", i);
                fs::write(file, data).unwrap();
            }
            eapply(&["files"]).unwrap();
        }
    }
}
