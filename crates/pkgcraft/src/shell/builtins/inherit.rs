use itertools::{Either, Itertools};
use scallop::builtins::ExecStatus;
use scallop::variables::{ScopedVariable, Variable, Variables};
use scallop::{functions, source, Error};

use crate::shell::get_build_mut;
use crate::types::Deque;

use super::Scopes::{Eclass, Global};
use super::{make_builtin, Scope};

const LONG_DOC: &str = "Sources the given list of eclasses.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let build = get_build_mut();
    // force incrementals to be restored between nested inherits
    let incrementals: Vec<(_, _)> = build
        .eapi()
        .incremental_keys()
        .iter()
        .map(|k| (*k, ScopedVariable::new(k)))
        .collect();

    let repo_eclasses = get_build_mut().repo()?.eclasses();
    let (eclasses, unknown): (Vec<_>, Vec<_>) = args
        .iter()
        // skip eclasses that have already been inherited
        .filter(|&s| !build.inherited.contains(*s))
        // map eclass args into known and unknown groups
        .partition_map(|&s| match repo_eclasses.get(s) {
            Some(v) => Either::Left(v),
            None => Either::Right(s),
        });

    // verify all eclass args are viable
    if !unknown.is_empty() {
        let s = unknown.join(", ");
        return Err(Error::Base(format!("unknown eclasses: {s}")));
    }

    let mut eclass_var = ScopedVariable::new("ECLASS");
    let mut inherited_var = Variable::new("INHERITED");
    let orig_scope = build.scope;

    // track direct inherits
    if orig_scope != Scope::Eclass {
        build.inherit.extend(eclasses.iter().map(|s| s.to_string()));
        build.scope = Scope::Eclass;
    }

    for eclass in eclasses {
        // skip inherits that occurred in nested calls
        if build.inherited.contains(eclass) {
            continue;
        }

        // mark as inherited before sourcing so nested, re-inherits can be skipped
        build.inherited.insert(eclass.to_string());

        // update $ECLASS bash variable
        eclass_var.bind(eclass, None, None)?;

        source::file(eclass.path()).map_err(|e| {
            // strip path prefix from bash error
            let s = e.to_string();
            let s = if s.starts_with('/') {
                match s.split_once(": ") {
                    Some((_, suffix)) => suffix,
                    None => s.as_str(),
                }
            } else {
                s.as_str()
            };
            Error::Base(format!("failed loading eclass: {eclass}: {s}"))
        })?;

        // append metadata keys that incrementally accumulate
        for (key, var) in &incrementals {
            if let Some(data) = var.string_vec() {
                build
                    .incrementals
                    .entry(*key)
                    .or_insert_with(Deque::new)
                    .extend(data);
            }
        }

        inherited_var.append(&format!(" {eclass}"))?;
    }

    // restore the original scope for non-nested contexts
    if orig_scope != Scope::Eclass {
        build.scope = orig_scope;

        // create aliases for EXPORT_FUNCTIONS calls
        for (phase, eclass) in build.export_functions.drain(..) {
            let func = format!("{eclass}_{phase}");
            if functions::find(&func).is_some() {
                source::string(format!("{phase}() {{ {func} \"$@\"; }}"))?;
            } else {
                return Err(Error::Base(format!("{eclass}: undefined phase function: {func}")));
            }
        }
    }

    Ok(ExecStatus::Success)
}

const USAGE: &str = "inherit eclass1 eclass2";
make_builtin!("inherit", inherit_builtin, run, LONG_DOC, USAGE, [("..", [Global, Eclass])]);

#[cfg(test)]
mod tests {
    use scallop::variables::{optional, string_vec};

    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::pkg::SourceablePackage;
    use crate::shell::BuildData;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as inherit;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(inherit, &[0]);
    }

    #[test]
    fn test_nonexistent() {
        let mut config = Config::default();
        let t = config.temp_repo("test1", 0, None).unwrap();

        // single
        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        let r = inherit(&["nonexistent"]);
        assert_err_re!(r, r"^unknown eclasses: nonexistent");

        // multiple
        let r = inherit(&["e1", "e2"]);
        assert_err_re!(r, r"^unknown eclasses: e1, e2");

        // multiple with known and unknown
        let t = config.temp_repo("test2", 0, None).unwrap();
        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        let eclass = indoc::indoc! {r#"
            # stub eclass
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let r = inherit(&["unknown1", "e1", "unknown2"]);
        assert_err_re!(r, r"^unknown eclasses: unknown1, unknown2");
    }

    #[test]
    fn test_source_failure() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // create eclass
        let eclass = indoc::indoc! {r#"
            # stub eclass
            unknown_cmd
        "#};
        t.create_eclass("e1", eclass).unwrap();

        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        let r = inherit(&["e1"]);
        assert_err_re!(r, r"^failed loading eclass: e1: unknown command: unknown_cmd$");
    }

    #[test]
    fn test_single() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // create eclass
        let eclass = indoc::indoc! {r#"
            # stub eclass
            [[ ${ECLASS} == e1 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e1", eclass).unwrap();

        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        inherit(&["e1"]).unwrap();
        assert_eq!(string_vec("INHERITED").unwrap(), ["e1"]);
    }

    #[test]
    fn test_multiple() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // create eclasses
        let eclass = indoc::indoc! {r#"
            # stub eclass
            [[ ${ECLASS} == e1 ]] || die "\$ECLASS isn't correct"
            inherit e2
            [[ ${ECLASS} == e1 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            [[ ${ECLASS} == e2 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e2", eclass).unwrap();

        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        inherit(&["e1"]).unwrap();
        assert_eq!(string_vec("INHERITED").unwrap(), ["e2", "e1"]);
    }

    #[test]
    fn test_nested_single() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // create eclasses
        let eclass = indoc::indoc! {r#"
            # stub eclass
            [[ ${ECLASS} == e1 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e1
            [[ ${ECLASS} == e2 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e2", eclass).unwrap();

        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        inherit(&["e2"]).unwrap();
        assert_eq!(string_vec("INHERITED").unwrap(), ["e1", "e2"]);
    }

    #[test]
    fn test_nested_multiple() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // create eclasses
        let eclass = indoc::indoc! {r#"
            # stub eclass
            [[ ${ECLASS} == e1 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e1
            [[ ${ECLASS} == e2 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e2", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e2
            [[ ${ECLASS} == e3 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e3", eclass).unwrap();

        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        inherit(&["e3"]).unwrap();
        assert_eq!(string_vec("INHERITED").unwrap(), ["e1", "e2", "e3"]);
    }

    #[test]
    fn test_pkg_env() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // create eclass
        let eclass = indoc::indoc! {r#"
            # stub eclass
            [[ ${ECLASS} == e1 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e1", eclass).unwrap();

        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing for eclass env transit"
            SLOT=0
            [[ -z ${ECLASS} ]] || die "\$ECLASS shouldn't be defined"
            [[ -n ${INHERITED} ]] || die "\$INHERITED should be defined"
        "#};
        let raw_pkg = t.create_raw_pkg_from_str("cat/pkg-1", data).unwrap();
        raw_pkg.source().unwrap();
    }

    #[test]
    fn test_skip_reinherits() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // create eclasses
        let eclass = indoc::indoc! {r#"
            # stub eclass
            VAR+="e1 "
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e1
            VAR+="e2"
        "#};
        t.create_eclass("e2", eclass).unwrap();

        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        inherit(&["e1", "e2"]).unwrap();
        assert_eq!(optional("VAR").unwrap(), "e1 e2");
    }
}
