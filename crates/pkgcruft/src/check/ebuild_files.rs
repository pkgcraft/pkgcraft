use std::path::Path;

use camino::Utf8Path;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::macros::build_path;
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::repo::Repository;
use tracing::warn;
use walkdir::WalkDir;

use crate::report::Location;
use crate::report::ReportKind::{FileUnknown, FilesUnused};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::{EbuildRawPkg, SourceKind};
use crate::Error;

use super::{CheckKind, EbuildRawPkgSetCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::EbuildFiles,
    scope: Scope::Package,
    source: SourceKind::EbuildRawPkg,
    reports: &[FileUnknown, FilesUnused],
    context: &[],
    priority: 0,
};

pub(super) fn create(repo: &'static EbuildRepo) -> impl EbuildRawPkgSetCheck {
    Check { repo }
}

struct Check {
    repo: &'static EbuildRepo,
}

/// Expand a variable into its actual value.
fn expand_var(
    pkg: &EbuildRawPkg,
    node: &crate::bash::Node,
    filesdir: &Utf8Path,
) -> crate::Result<String> {
    let mut var_node = None;
    let mut nodes = vec![];
    for x in node {
        if x.kind() == "variable_name" {
            var_node = Some(x);
        }
        nodes.push(x);
    }

    let err = |msg: &str| {
        let location = Location::from(node);
        Err(Error::InvalidValue(format!("{location}: {msg}: {node}")))
    };

    // TODO: handle string substitution
    if nodes.len() > 3 {
        return err("unhandled string expansion");
    }

    let Some(var) = var_node else {
        return err("invalid variable node");
    };

    let cpv = pkg.cpv();
    match var.as_str() {
        "FILESDIR" => Ok(filesdir.to_string()),
        "CATEGORY" => Ok(cpv.category().to_string()),
        "PN" => Ok(cpv.package().to_string()),
        "P" => Ok(cpv.p().to_string()),
        "PF" => Ok(cpv.pf().to_string()),
        "PR" => Ok(cpv.pr().to_string()),
        "PV" => Ok(cpv.pv().to_string()),
        "PVR" => Ok(cpv.pvr().to_string()),
        // TODO: source ebuild and extract environment variables
        _ => err("unhandled variable"),
    }
}

/// Resolve all variables in a parse tree node, returning the string value.
fn expand_node<'a>(
    pkg: &EbuildRawPkg,
    node: &crate::bash::Node<'a>,
    cursor: &mut tree_sitter::TreeCursor<'a>,
    filesdir: &Utf8Path,
) -> crate::Result<String> {
    let mut path = String::new();
    for x in node.children(cursor) {
        match x.kind() {
            "expansion" | "simple_expansion" => match expand_var(pkg, &x, filesdir) {
                Ok(value) => path.push_str(&value),
                Err(e) => return Err(e),
            },
            "string" => match expand_node(pkg, &x, cursor, filesdir) {
                Ok(value) => path.push_str(&value),
                Err(e) => return Err(e),
            },
            "word" | "string_content" => path.push_str(x.as_str()),
            "\"" => (),
            kind => {
                let location = Location::from(&x);
                return Err(Error::InvalidValue(format!(
                    "{location}: unhandled node variant: {kind}: {x}"
                )));
            }
        }
    }
    Ok(path)
}

impl EbuildRawPkgSetCheck for Check {
    fn run(&self, cpn: &Cpn, pkgs: &[EbuildRawPkg], filter: &mut ReportFilter) {
        let filesdir = build_path!(self.repo.path(), cpn.category(), cpn.package(), "files");
        // TODO: flag non-utf8 file names?
        let mut files: IndexSet<_> = WalkDir::new(&filesdir)
            .min_depth(1)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .map(|e| e.path().to_str().unwrap().to_string())
            .collect();
        let mut used_files = IndexSet::new();

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
                        // expand references
                        let mut path = match expand_node(pkg, &node, &mut cursor, &filesdir) {
                            Ok(path) => path,
                            Err(e) => {
                                warn!("{CHECK}: {pkg}: {e}");
                                return;
                            }
                        };

                        // handle strings with embedded $FILESDIR usage
                        if !path.starts_with(filesdir.as_str()) {
                            if let Some(idx) = path.find(filesdir.as_str()) {
                                path = path.split_at(idx).1.to_string();
                            } else {
                                warn!("{CHECK}: {pkg}: unhandled file path: {path}");
                                return;
                            }
                        }

                        // flag nonexistent files
                        let mut is_unknown = |path: &str| {
                            if let Some(value) = files.swap_take(path) {
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
                                            .location(&node)
                                            .report(filter);
                                    }
                                }
                            }
                        };

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
            files.sort();
            let files = files
                .iter()
                .filter_map(|x| x.strip_prefix(filesdir.as_str()))
                .map(|x| x.trim_start_matches('/'))
                .join(", ");
            FilesUnused.package(cpn).message(files).report(filter);
        }
    }
}
