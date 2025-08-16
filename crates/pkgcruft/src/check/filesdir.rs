use std::collections::HashSet;
use std::fs;
use std::path::Path;

use camino::Utf8Path;
use itertools::Itertools;
use pkgcraft::bash::{Node, Tree};
use pkgcraft::dep::Cpn;
use pkgcraft::macros::build_path;
use pkgcraft::pkg::{Package, ebuild::EbuildPkg};
use pkgcraft::repo::ebuild::Eclass;
use rayon::prelude::*;
use tracing::warn;
use walkdir::WalkDir;

use crate::Error;
use crate::report::Location;
use crate::report::ReportKind::{FileUnknown, FilesUnused};
use crate::scan::ScannerRun;

use super::EbuildPkgSetCheck;

pub(super) fn create(run: &ScannerRun) -> impl EbuildPkgSetCheck + 'static {
    let eclasses = run
        .repo
        .eclasses()
        .into_par_iter()
        .filter_map(|e| {
            if let Ok(data) = fs::read_to_string(e.path()) {
                if Tree::new(data.into())
                    .into_iter()
                    .any(|x| x.kind() == "variable_name" && x.as_str() == "FILESDIR")
                {
                    return Some(e);
                }
            }
            None
        })
        .cloned()
        .collect();

    Check { eclasses }
}

struct Check {
    eclasses: HashSet<Eclass>,
}

super::register!(Check, super::Check::Filesdir);

/// Expand a variable into its actual value.
fn expand_var<'a>(
    pkg: &'a EbuildPkg,
    node: Node<'a>,
    cursor: &mut tree_sitter::TreeCursor<'a>,
    filesdir: &Utf8Path,
) -> crate::Result<String> {
    let error = |msg: &str| Error::InvalidValue(format!("expanding {node}: {msg}"));
    let cpv = pkg.cpv();

    let mut var_node = None;
    if node.kind() == "variable_name" {
        var_node = Some(node);
    } else {
        let mut nodes = vec![];

        for x in node {
            if x.kind() == "variable_name" {
                var_node = Some(x);
            }
            nodes.push(x);
        }

        // TODO: handle string substitution
        if nodes.len() > 3 {
            return Err(error("unhandled string expansion"));
        }
    }

    let Some(var) = var_node else {
        return Err(error("invalid variable node"));
    };

    match var.as_str() {
        "FILESDIR" => Ok(filesdir.to_string()),
        "CATEGORY" => Ok(cpv.category().to_string()),
        "PN" => Ok(cpv.package().to_string()),
        "P" => Ok(cpv.p().to_string()),
        "PF" => Ok(cpv.pf().to_string()),
        "PR" => Ok(cpv.pr().to_string()),
        "PV" => Ok(cpv.pv().to_string()),
        "PVR" => Ok(cpv.pvr().to_string()),
        "SLOT" => Ok(pkg.slot().to_string()),
        // search and expand unknown variables
        name => {
            // TODO: consider caching globally defined variables during metadata gen for lookups
            // TODO: determine if the variable is globally defined before looking for it?
            let Some(node) = pkg
                .tree()
                .iter_global()
                .filter(|node| node.kind() == "variable_assignment")
                .find(|node| node.name().map(|x| x == name).unwrap_or_default())
            else {
                return Err(error("unhandled local variable"));
            };

            if let Some(val) = node.into_iter().nth(2) {
                match expand_node(pkg, val, cursor, filesdir) {
                    Ok(val) => Ok(val),
                    Err(e) => Err(error(&format!("{node}: unhandled global variable: {e}"))),
                }
            } else {
                Err(error(&format!("{node}: invalid assignment")))
            }
        }
    }
}

/// Resolve all variables in a parse tree node, returning the string value.
fn expand_node<'a>(
    pkg: &'a EbuildPkg,
    node: Node<'a>,
    cursor: &mut tree_sitter::TreeCursor<'a>,
    filesdir: &Utf8Path,
) -> crate::Result<String> {
    let mut path = String::new();
    let mut nodes: Vec<_> = node.children(cursor).collect();
    // handle static node variants like number or word
    if nodes.is_empty() {
        nodes.push(node);
    }

    for x in nodes {
        match x.kind() {
            "expansion" | "simple_expansion" | "variable_name" => {
                match expand_var(pkg, x, cursor, filesdir) {
                    Ok(value) => path.push_str(&value),
                    Err(e) => return Err(e),
                }
            }
            "string" => match expand_node(pkg, x, cursor, filesdir) {
                Ok(value) => path.push_str(&value),
                Err(e) => return Err(e),
            },
            "word" | "string_content" | "number" => path.push_str(x.as_str()),
            "\"" | "${" | "}" => continue,
            kind => {
                return Err(Error::InvalidValue(format!(
                    "unhandled node variant: {kind}: {x}"
                )));
            }
        }
    }
    Ok(path)
}

impl EbuildPkgSetCheck for Check {
    fn run(&self, cpn: &Cpn, pkgs: &[EbuildPkg], run: &ScannerRun) {
        let filesdir = build_path!(run.repo.path(), cpn.category(), cpn.package(), "files");
        // TODO: flag non-utf8 file names?
        let mut files: HashSet<_> = WalkDir::new(&filesdir)
            .min_depth(1)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().is_file())
            .map(|e| e.path().to_string_lossy().to_string())
            .collect();
        let mut used_files = HashSet::new();

        for pkg in pkgs {
            let mut cursor = pkg.tree().walk();
            for node in pkg.tree() {
                if node.kind() == "variable_name" && node.as_str() == "FILESDIR" {
                    let mut target = node.parent();
                    while let Some(node) = target {
                        if node.kind() == "concatenation"
                            || (node.kind() == "string"
                                && node
                                    .parent()
                                    .map(|x| x.kind() != "concatenation")
                                    .unwrap_or_default())
                        {
                            break;
                        }
                        target = node.parent();
                    }
                    if let Some(node) = target {
                        let location = Location::from(&node);

                        // expand references
                        let mut path = match expand_node(pkg, node, &mut cursor, &filesdir) {
                            Ok(path) => path,
                            Err(e) => {
                                warn!("{self}: {pkg}, {location}: {node}: {e}");
                                // disable FilesUnused report
                                files.clear();
                                continue;
                            }
                        };

                        // handle strings with prefixed $FILESDIR usage
                        if !path.starts_with(filesdir.as_str()) {
                            let idx = path.find(filesdir.as_str()).unwrap_or_else(|| {
                                panic!("{self}: {pkg}, {location}: failed expanding: {node}")
                            });
                            path = path.split_at(idx).1.to_string();
                        }

                        // flag nonexistent files
                        let mut is_unknown = |path: &str| {
                            if let Some(value) = files.take(path) {
                                used_files.insert(value);
                            } else if !used_files.contains(path)
                                && !Path::new(path).exists()
                                && !node.in_conditional()
                            {
                                if let Some(file) = path.strip_prefix(filesdir.as_str()) {
                                    if file.starts_with('/') {
                                        FileUnknown
                                            .version(pkg)
                                            .message(file.trim_start_matches('/'))
                                            .location(location)
                                            .report(run);
                                    }
                                }
                            }
                        };

                        // consider skipping all reports when any conditionals are found to avoid
                        // false positives

                        // expand dir path to all files
                        if Path::new(&path).is_dir() {
                            path = format!("{}/*", path.trim_end_matches('/'));
                        }

                        // expand paths using bash's expansion support
                        for path in scallop::variables::expand_iter([&path]) {
                            is_unknown(&path);
                        }
                    }
                }
            }
        }

        if !files.is_empty() {
            // ignore unused files if inherited eclasses use FILESDIR
            let inherited: HashSet<_> =
                pkgs.iter().flat_map(|x| x.inherited()).cloned().collect();
            if let Some(eclass) = self.eclasses.intersection(&inherited).next() {
                warn!("{self}: {cpn}: skipping unused files due to eclass FILESDIR: {eclass}");
                return;
            }

            let files = files
                .iter()
                .filter_map(|x| x.strip_prefix(filesdir.as_str()))
                .map(|x| x.trim_start_matches('/'))
                .sorted()
                .join(", ");
            FilesUnused.package(cpn).message(files).report(run);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::{test_data, test_data_patched};

    use crate::scan::Scanner;
    use crate::test::{assert_unordered_reports, glob_reports};

    use super::*;

    #[test]
    fn check() {
        let scanner = Scanner::new().reports([CHECK]);

        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
