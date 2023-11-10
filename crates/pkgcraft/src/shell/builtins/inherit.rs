use scallop::variables::{ScopedVariable, Variable, Variables};
use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;
use crate::traits::SourceBash;

use super::export_functions::export_functions;
use super::make_builtin;

const LONG_DOC: &str = "Sources the given list of eclasses.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
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

    let repo_eclasses = build.repo()?.eclasses();
    let mut eclass_var = ScopedVariable::new("ECLASS");
    let mut inherited_var = Variable::new("INHERITED");

    for name in args {
        let eclass = repo_eclasses
            .get(*name)
            .ok_or_else(|| Error::Base(format!("unknown eclass: {name}")))?;

        // track all inherits
        if !build.inherited.insert(eclass) {
            // skip previous and nested inherits
            continue;
        }

        // track direct inherits
        if !build.scope.is_eclass() {
            build.inherit.insert(eclass);
        }

        // track build scope
        let _scope = build.inherit(eclass);

        // update $ECLASS bash variable
        eclass_var.bind(eclass, None, None)?;

        eclass.source_bash().map_err(|e| {
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
                build.incrementals.entry(*key).or_default().extend(data);
            }
        }

        inherited_var.append(format!(" {eclass}"))?;
    }

    // create function aliases for EXPORT_FUNCTIONS calls
    if !build.scope.is_eclass() {
        export_functions(build.export_functions.drain(..))?;
    }

    Ok(ExecStatus::Success)
}

const USAGE: &str = "inherit eclass1 eclass2";
make_builtin!("inherit", inherit_builtin);

#[cfg(test)]
mod tests {
    use scallop::variables::{optional, string_vec};

    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::pkg::SourcePackage;
    use crate::shell::BuildData;

    use super::super::{assert_invalid_args, builtin_scope_tests, inherit};
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
        assert_err_re!(r, r"^unknown eclass: nonexistent");

        // multiple
        let r = inherit(&["e1", "e2"]);
        assert_err_re!(r, r"^unknown eclass: e1");

        // multiple with existing and nonexistent
        let t = config.temp_repo("test2", 0, None).unwrap();
        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        let eclass = indoc::indoc! {r#"
            # stub eclass
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let r = inherit(&["e1", "e2"]);
        assert_err_re!(r, r"^unknown eclass: e2");
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
        assert_err_re!(r, "^failed loading eclass: e1: line 2: unknown command: unknown_cmd$");
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
        "#};
        let raw_pkg = t.create_raw_pkg_from_str("cat/pkg-1", data).unwrap();
        raw_pkg.source().unwrap();
        assert!(optional("ECLASS").is_none(), "$ECLASS shouldn't be defined");
        assert_eq!(string_vec("INHERITED").unwrap(), ["e1"]);
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
        let mut var = Variable::new("VAR");

        // verify previous inherits are skipped
        BuildData::from_raw_pkg(&raw_pkg);
        inherit(&["e1", "e2"]).unwrap();
        assert_eq!(var.optional().unwrap(), "e1 e2");

        // verify nested inherits are skipped
        BuildData::from_raw_pkg(&raw_pkg);
        var.unbind().unwrap();
        inherit(&["e2", "e1"]).unwrap();
        assert_eq!(var.optional().unwrap(), "e1 e2");
    }
}
