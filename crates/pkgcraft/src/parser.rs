#![warn(clippy::pedantic)]
#![warn(unused_imports)]

use winnow::{
    ascii::{alpha1, digit1, multispace1},
    combinator::{
        alt, cond, cut_err, delimited, dispatch, empty, eof, fail, not, opt, peek, preceded,
        repeat, repeat_till, separated, seq, terminated, trace,
    },
    error::{AddContext, ContextError, ErrMode, ParserError, StrContext, StrContextValue},
    prelude::*,
    stream::{AsChar, ContainsToken, ParseSlice, Stream, StreamIsPartial},
    token::{any, one_of, take_till, take_while},
};

use crate::{
    dep::{
        version::{Number, Suffix, SuffixKind, WithOp},
        Blocker, Cpn, Cpv, CpvOrDep, Dep, Dependency, DependencySet, Operator, Revision, Slot,
        SlotDep, SlotOperator, Uri, UseDep, UseDepKind, Version,
    },
    eapi::{Eapi, Feature},
    pkg::ebuild::{
        iuse::Iuse,
        keyword::{Keyword, KeywordStatus},
    },
    types::{Ordered, SortedSet},
};

pub(crate) fn blocker(input: &mut &str) -> ModalResult<Blocker> {
    trace("blocker", ('!', opt('!')).take().parse_to()).parse_next(input)
}

pub(crate) fn slot_dep(input: &mut &str) -> ModalResult<SlotDep> {
    trace(
        "slot_dep",
        dispatch!(opt(alt(('=', '*'))).take();
            "=" => empty.value(SlotDep::Op(SlotOperator::Equal)),
            "*" => empty.value(SlotDep::Op(SlotOperator::Star)),
            "" => (slot, opt("="))
                .map(|(slot, op)| if op.is_some() {
                    SlotDep::SlotOp(slot, SlotOperator::Equal)
                } else {
                    SlotDep::Slot(slot)
                }),
            _ => fail,
        ),
    )
    .parse_next(input)
}

pub(crate) fn slot_dep_str(input: &mut &str) -> ModalResult<SlotDep> {
    trace("slot_dep_str", preceded(':', slot_dep)).parse_next(input)
}

pub(crate) fn iuse(input: &mut &str) -> ModalResult<Iuse> {
    trace(
        "iuse",
        seq!(Iuse {
            default: opt(alt(('+'.value(true), '-'.value(false)))),
            flag: use_flag_name.map(str::to_string),
        }),
    )
    .parse_next(input)
}

fn use_dep_default(input: &mut &str) -> ModalResult<bool> {
    trace("use_dep_default", delimited('(', alt(('+'.value(true), '-'.value(false))), ')'))
        .parse_next(input)
}

pub(crate) fn use_dep(input: &mut &str) -> ModalResult<UseDep> {
    trace("use_dep", |input: &mut &str| {
        let disabled = opt(alt(('!', '-'))).parse_next(input)?;
        let flag = use_flag_name.map(str::to_string).parse_next(input)?;
        let default = opt(use_dep_default).parse_next(input)?;
        let kind = if let Some(disabled) = disabled {
            if disabled == '!' {
                alt(('='.value(UseDepKind::Equal), '?'.value(UseDepKind::Conditional)))
                    .parse_next(input)?
            } else {
                UseDepKind::Enabled
            }
        } else {
            opt(alt(('='.value(UseDepKind::Equal), '?'.value(UseDepKind::Conditional))))
                .parse_next(input)?
                .unwrap_or(UseDepKind::Enabled)
        };
        Ok(UseDep {
            flag,
            kind,
            enabled: disabled.is_none(),
            default,
        })
    })
    .parse_next(input)
}

fn use_deps(input: &mut &str) -> ModalResult<SortedSet<UseDep>> {
    trace("use_deps", delimited('[', separated(1.., use_dep, ','), ']')).parse_next(input)
}

fn repo_dep<'i>(eapi: &'static Eapi) -> impl ModalParser<&'i str, &'i str, ContextError> {
    trace("repo_dep", move |input: &mut &'i str| {
        let start = input.checkpoint();
        preceded("::", cond(eapi.has(Feature::RepoIds), repository_name))
            .parse_next(input)?
            .ok_or_else(|| {
                ErrMode::from_input(input).cut().add_context(
                    input,
                    &start,
                    StrContext::Expected(StrContextValue::Description(
                        "Eapi doesn't support repository ids",
                    )),
                )
            })
    })
}

fn dep_op_pkg(input: &mut &str) -> ModalResult<Dep> {
    trace("dep_op_pkg", |input: &mut &str| {
        let mut op = operator.parse_next(input)?;
        let Cpv { cpn, version } = cpv.parse_next(input)?;
        if op == Operator::Equal && opt('*').parse_next(input)?.is_some() {
            op = Operator::EqualGlob;
        }
        Ok(Dep {
            cpn,
            version: version.with_op(op).ok(),
            blocker: None,
            slot_dep: None,
            use_deps: None,
            repo: None,
        })
    })
    .parse_next(input)
}

fn dep_pkg(input: &mut &str) -> ModalResult<Dep> {
    trace("dep_pkg", alt((cpn.map(Into::into), dep_op_pkg))).parse_next(input)
}

pub(crate) fn dep<'i>(eapi: &'static Eapi) -> impl ModalParser<&'i str, Dep, ContextError> {
    trace("dep", move |input: &mut &'i str| {
        let (blocker, Dep { cpn, version, .. }, slot_dep, repo, use_deps) = (
            opt(blocker),
            dep_pkg,
            opt(slot_dep_str),
            opt(repo_dep(eapi).map(str::to_string)),
            opt(use_deps),
        )
            .parse_next(input)?;
        Ok(Dep {
            cpn,
            blocker,
            version,
            slot_dep,
            use_deps,
            repo,
        })
    })
}

pub(crate) fn cpv_or_dep(input: &mut &str) -> ModalResult<CpvOrDep> {
    trace(
        "cpv_or_dep",
        alt((cpv.map(CpvOrDep::Cpv), dep(Default::default()).map(CpvOrDep::Dep))),
    )
    .parse_next(input)
}

pub(crate) fn cpv(input: &mut &str) -> ModalResult<Cpv> {
    trace(
        "cpv",
        seq!(Cpv {
            cpn: cpn,
            _: '-',
            version: version,
        }),
    )
    .parse_next(input)
}

pub(crate) fn cpn(input: &mut &str) -> ModalResult<Cpn> {
    trace(
        "cpn",
        seq!(Cpn {
            category: category_name.map(str::to_string),
            _: '/',
            package: package_name.map(str::to_string),
        }),
    )
    .parse_next(input)
}

pub(crate) fn slot(input: &mut &str) -> ModalResult<Slot> {
    trace("slot", (slot_name, opt(('/', slot_name))).take())
        .parse_next(input)
        .map(str::to_string)
        .map(|name| Slot { name })
}

pub(crate) fn keyword(input: &mut &str) -> ModalResult<Keyword> {
    trace("keyword", move |input: &mut &str| {
        let status =
            opt(alt(("-".value(KeywordStatus::Disabled), "~".value(KeywordStatus::Unstable))))
                .map(|status| status.unwrap_or(KeywordStatus::Stable))
                .parse_next(input)?;
        let arch =
            alt(("*".verify(|_: &str| status == KeywordStatus::Disabled), keyword_name))
                .parse_next(input)?;
        Ok(Keyword { status, arch: arch.into() })
    })
    .parse_next(input)
}

pub(crate) fn eapi_value<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    trace(
        "eapi_value",
        alt((eapi_name, delimited('"', eapi_name, '"'), delimited('\'', eapi_name, '\''))),
    )
    .parse_next(input)
}

pub(crate) fn license_dependency_set(input: &mut &str) -> ModalResult<DependencySet<String>> {
    separated(0.., license_dependency, multispace1).parse_next(input)
}

pub(crate) fn license_dependency(input: &mut &str) -> ModalResult<Dependency<String>> {
    alt((
        conditional(license_dependency),
        any_of(license_dependency),
        all_of(license_dependency),
        license_name.map(str::to_string).map(Dependency::Enabled),
    ))
    .parse_next(input)
}

pub(crate) fn src_uri_dependency_set(input: &mut &str) -> ModalResult<DependencySet<Uri>> {
    trace("src_uri_dependency_set", separated(0.., src_uri_dependency, multispace1))
        .parse_next(input)
}

pub(crate) fn src_uri_dependency(input: &mut &str) -> ModalResult<Dependency<Uri>> {
    trace(
        "src_uri_dependency",
        alt((
            conditional(src_uri_dependency),
            all_of(src_uri_dependency),
            src_uri_dependency_,
        )),
    )
    .parse_next(input)
}

fn src_uri_dependency_(input: &mut &str) -> ModalResult<Dependency<Uri>> {
    trace("src_uri_dependency_", move |input: &mut &str| {
        (
            preceded(not(')'), take_till(1.., (AsChar::is_space, AsChar::is_newline))),
            opt(preceded(
                (multispace1, "->", multispace1),
                take_till(1.., (AsChar::is_space, AsChar::is_newline)),
            )),
        )
            .parse_next(input)
            .map(|(uri, rename)| Uri::new(uri, rename))
            .map(Dependency::Enabled)
    })
    .parse_next(input)
}

pub(crate) fn properties_dependency_set(
    input: &mut &str,
) -> ModalResult<DependencySet<String>> {
    separated(0.., properties_dependency, multispace1).parse_next(input)
}

pub(crate) fn properties_dependency(input: &mut &str) -> ModalResult<Dependency<String>> {
    trace(
        "properties_dependency",
        alt((
            conditional(properties_dependency),
            all_of(properties_dependency),
            license_name.map(str::to_string).map(Dependency::Enabled),
        )),
    )
    .parse_next(input)
}
pub(crate) fn required_use_dependency_set(
    input: &mut &str,
) -> ModalResult<DependencySet<String>> {
    trace(
        "required_use_dependency_set",
        separated(0.., required_use_dependency, multispace1),
    )
    .parse_next(input)
}

pub(crate) fn required_use_dependency(input: &mut &str) -> ModalResult<Dependency<String>> {
    trace(
        "required_use_dependency",
        alt((
            conditional(required_use_dependency),
            any_of(required_use_dependency),
            all_of(required_use_dependency),
            exactly_one_of(required_use_dependency),
            at_most_one_of(required_use_dependency),
            required_use_dependency_,
        )),
    )
    .parse_next(input)
}

fn required_use_dependency_(input: &mut &str) -> ModalResult<Dependency<String>> {
    trace("required_use_dependency_", move |input: &mut &str| {
        let disabled = opt('!').parse_next(input)?;
        let use_flag = use_flag_name.map(str::to_string).parse_next(input)?;
        if disabled.is_some() {
            Ok(Dependency::Disabled(use_flag))
        } else {
            Ok(Dependency::Enabled(use_flag))
        }
    })
    .parse_next(input)
}

pub(crate) fn restrict_dependency_set(input: &mut &str) -> ModalResult<DependencySet<String>> {
    separated(0.., restrict_dependency, multispace1).parse_next(input)
}

pub(crate) fn restrict_dependency(input: &mut &str) -> ModalResult<Dependency<String>> {
    alt((
        conditional(restrict_dependency),
        all_of(restrict_dependency),
        license_name.map(str::to_string).map(Dependency::Enabled),
    ))
    .parse_next(input)
}

pub(crate) fn package_dependency_set<'i>(
    eapi: &'static Eapi,
) -> impl ModalParser<&'i str, DependencySet<Dep>, ContextError> {
    separated(0.., package_dependency(eapi), multispace1)
}

pub(crate) fn package_dependency<'i>(
    eapi: &'static Eapi,
) -> impl ModalParser<&'i str, Dependency<Dep>, ContextError> {
    move |input: &mut &str| {
        alt((
            conditional(package_dependency(eapi)),
            any_of(package_dependency(eapi)),
            all_of(package_dependency(eapi)),
            dep(eapi).map(Dependency::Enabled),
        ))
        .parse_next(input)
    }
}

fn conditional<'i, O>(
    mut parser: impl ModalParser<&'i str, Dependency<O>, ContextError>,
) -> impl ModalParser<&'i str, Dependency<O>, ContextError>
where
    O: Ordered,
{
    move |input: &mut &'i str| {
        let (disabled, flag, _, _) =
            (opt('!'), use_flag_name, '?', multispace1).parse_next(input)?;
        let use_dep = UseDep {
            flag: flag.to_string(),
            kind: UseDepKind::Conditional,
            enabled: disabled.is_none(),
            default: None,
        };
        let dependencies = group(parser.by_ref()).parse_next(input)?;
        let dependencies = dependencies.into_iter().map(Box::new).collect();
        Ok(Dependency::Conditional(use_dep, dependencies))
    }
}

fn all_of<'i, O, E>(
    mut parser: impl Parser<&'i str, Dependency<O>, E>,
) -> impl Parser<&'i str, Dependency<O>, E>
where
    O: Ordered,
    E: ParserError<&'i str>,
{
    move |input: &mut &'i str| {
        let dependencies = group(parser.by_ref()).parse_next(input)?;
        let dependencies = dependencies.into_iter().map(Box::new).collect();
        Ok(Dependency::AllOf(dependencies))
    }
}

fn any_of<'i, O, E>(
    mut parser: impl Parser<&'i str, Dependency<O>, E>,
) -> impl Parser<&'i str, Dependency<O>, E>
where
    O: Ordered,
    E: ParserError<&'i str>,
{
    move |input: &mut &'i str| {
        let dependencies =
            preceded(("||", multispace1), group(parser.by_ref())).parse_next(input)?;
        let dependencies = dependencies.into_iter().map(Box::new).collect();
        Ok(Dependency::AnyOf(dependencies))
    }
}

fn exactly_one_of<'i, O, E>(
    mut parser: impl Parser<&'i str, Dependency<O>, E>,
) -> impl Parser<&'i str, Dependency<O>, E>
where
    O: Ordered,
    E: ParserError<&'i str>,
{
    move |input: &mut &'i str| {
        let dependencies =
            preceded(("^^", multispace1), group(parser.by_ref())).parse_next(input)?;
        let dependencies = dependencies.into_iter().map(Box::new).collect();
        Ok(Dependency::ExactlyOneOf(dependencies))
    }
}

fn at_most_one_of<'i, O, E>(
    mut parser: impl Parser<&'i str, Dependency<O>, E>,
) -> impl Parser<&'i str, Dependency<O>, E>
where
    O: Ordered,
    E: ParserError<&'i str>,
{
    move |input: &mut &'i str| {
        let dependencies =
            preceded(("??", multispace1), group(parser.by_ref())).parse_next(input)?;
        let dependencies = dependencies.into_iter().map(Box::new).collect();
        Ok(Dependency::AtMostOneOf(dependencies))
    }
}

fn group<'i, O, E>(parser: impl Parser<&'i str, O, E>) -> impl Parser<&'i str, Vec<O>, E>
where
    E: ParserError<&'i str>,
{
    trace(
        "group",
        delimited(('(', multispace1), repeat(1.., terminated(parser, multispace1)), ')'),
    )
}

// 3.1.1 Category names
pub(crate) fn category_name<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    trace("category_name", name((AsChar::is_alphanum, '_'), ('-', '.', '+'))).parse_next(input)
}

// 3.1.2 Package names
pub(crate) fn package_name<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    trace(
        "package_name",
        (
            one_of((AsChar::is_alphanum, '_')),
            repeat::<_, _, Vec<_>, _, _>(
                0..,
                alt((
                    one_of((AsChar::is_alphanum, '_', '+')).take(),
                    // TODO: investigate how this effects performance
                    terminated(
                        "-",
                        not((
                            separated::<_, _, Vec<_>, _, _, _, _>(1.., version, '-').void(),
                            alt(("*", ":", "[", eof)),
                        )),
                    ),
                )),
            )
            .take(),
        )
            .take(),
    )
    .parse_next(input)
}

// 3.1.3 Slot names
pub(crate) fn slot_name<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    trace("slot_name", name((AsChar::is_alphanum, '_'), ('-', '.', '+'))).parse_next(input)
}

// 3.1.4 USE flag names
pub(crate) fn use_flag_name<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    trace("use_flag_name", name(AsChar::is_alphanum, ('_', '-', '.', '+', '@')))
        .parse_next(input)
}

// 3.1.5 Repository names
pub(crate) fn repository_name<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    trace(
        "repository_name",
        (
            one_of((AsChar::is_alphanum, '_')),
            repeat::<_, _, Vec<_>, _, _>(
                0..,
                alt((
                    one_of((AsChar::is_alphanum, '_')).take(),
                    // TODO: investigate how this effects performance
                    terminated(
                        "-",
                        not((
                            separated::<_, _, Vec<_>, _, _, _, _>(1.., version, '-').void(),
                            alt(("*", ":", "[", eof)),
                        )),
                    ),
                )),
            )
            .take(),
        )
            .take(),
    )
    .parse_next(input)
}

// 3.1.6 Eclass names
pub(crate) fn eclass_name<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    trace(
        "eclass_name",
        preceded(
            not("default"),
            name((AsChar::is_alpha, '_'), (AsChar::is_dec_digit, '-', '.')),
        ),
    )
    .parse_next(input)
}

// 3.1.7 License names
pub(crate) fn license_name<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    trace("license_name", name((AsChar::is_alphanum, '_'), ('-', '.', '+'))).parse_next(input)
}

/// 3.1.8
pub(crate) fn keyword_name<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    trace("keyword_name", name((AsChar::is_alphanum, '_'), '-')).parse_next(input)
}

// 3.1.9 Eapi names
pub(crate) fn eapi_name<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    trace("eapi_name", name((AsChar::is_alphanum, '_'), ('-', '.', '+'))).parse_next(input)
}

/// Take a single prefix token followed by one or more prefix + suffix tokens.
// TODO: pass context as arguments OR delete function
pub(crate) fn name<I, E, Prefix, Suffix>(
    prefix: Prefix,
    suffix: Suffix,
) -> impl Parser<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial + Stream,
    <I as Stream>::Token: Clone,
    E: ParserError<I>,
    Prefix: ContainsToken<<I as Stream>::Token> + Clone,
    Suffix: ContainsToken<<I as Stream>::Token> + Clone,
{
    trace("name", (one_of(prefix.clone()), take_while(0.., (prefix, suffix))).take())
}

pub(crate) fn version_with_op(input: &mut &str) -> ModalResult<Version> {
    trace("version_with_op", move |input: &mut &str| {
        let Version {
            mut op,
            numbers,
            letter,
            suffixes,
            revision,
        } = seq!(Version {
            op: operator.map(Some),
            numbers: numbers,
            letter: opt(letter),
            suffixes: suffixes,
            revision: revision,
        })
        .verify(|version| {
            // The approximate operator may only be used on package names without a revision
            if version
                .op
                .as_ref()
                .is_some_and(|op| *op == Operator::Approximate)
            {
                return version.revision.is_empty();
            }
            true
        })
        .parse_next(input)?;

        // Transform the Equal operator to EqualGlob if the version has a trailing '*'
        if op.as_ref().is_some_and(|op| *op == Operator::Equal)
            && opt('*').parse_next(input)?.is_some()
        {
            op = Some(Operator::EqualGlob);
        }

        Ok(Version {
            op,
            numbers,
            letter,
            suffixes,
            revision,
        })
    })
    .parse_next(input)
}

pub(crate) fn version(input: &mut &str) -> ModalResult<Version> {
    trace(
        "version",
        seq!(Version {
            op: empty.value(None),
            numbers: numbers,
            letter: opt(letter),
            suffixes: suffixes,
            revision: revision,
        }),
    )
    .parse_next(input)
}

pub(crate) fn operator(input: &mut &str) -> ModalResult<Operator> {
    trace(
        "operator",
        dispatch!(take_while(1..=2, ('<', '=', '>', '~'));
            "<=" => empty.value(Operator::LessOrEqual),
            ">=" => empty.value(Operator::GreaterOrEqual),
            "<" => empty.value(Operator::Less),
            "=" => empty.value(Operator::Equal),
            "~" => empty.value(Operator::Approximate),
            ">" => empty.value(Operator::Greater),
            _ => fail,
        ),
    )
    .parse_next(input)
}

pub(crate) fn numbers(input: &mut &str) -> ModalResult<Vec<Number>> {
    trace("numbers", separated(1.., number, '.')).parse_next(input)
}

pub(crate) fn letter(input: &mut &str) -> ModalResult<char> {
    trace("letter", one_of('a'..='z')).parse_next(input)
}

pub(crate) fn suffixes(input: &mut &str) -> ModalResult<Vec<Suffix>> {
    trace("suffixes", repeat(0.., suffix)).parse_next(input)
}

pub(crate) fn suffix(input: &mut &str) -> ModalResult<Suffix> {
    trace(
        "suffix",
        seq!(Suffix {
            _: '_',
            kind: suffix_kind,
            version: opt(number),
        }),
    )
    .parse_next(input)
}

pub(crate) fn suffix_kind(input: &mut &str) -> ModalResult<SuffixKind> {
    trace(
        "suffix_kind",
        dispatch!(alpha1;
            "alpha" => empty.value(SuffixKind::Alpha),
            "beta" => empty.value(SuffixKind::Beta),
            "pre" => empty.value(SuffixKind::Pre),
            "rc" => empty.value(SuffixKind::Rc),
            "p" => empty.value(SuffixKind::P),
            _ => fail,
        ),
    )
    .parse_next(input)
}

pub(crate) fn revision(input: &mut &str) -> ModalResult<Revision> {
    trace("revision", opt(preceded("-r", number)))
        .parse_next(input)
        .map(Option::unwrap_or_default)
        .map(Revision)
}

pub(crate) fn number(input: &mut &str) -> ModalResult<Number> {
    trace("number", |input: &mut &str| {
        let start = input.checkpoint();
        let raw = digit1.parse_next(input)?;
        let value = raw.parse_slice().ok_or_else(|| {
            input.reset(&start);
            ParserError::from_input(input)
        })?;
        Ok(Number { raw: raw.to_string(), value })
    })
    .parse_next(input)
}

#[cfg(test)]
mod tests {
    use winnow::error::ContextError;

    use super::*;

    #[test]
    fn eapi_values() {
        assert_eq!(eapi_value.parse("8"), Ok("8"));
        assert_eq!(eapi_value.parse("_foo"), Ok("_foo"));
        assert_eq!(eapi_value.parse("Foo"), Ok("Foo"));
    }

    #[test]
    fn package_names() {
        assert_eq!(package_name.parse_peek("pkg-1::repo"), Ok(("-1::repo", "pkg")));
    }

    #[test]
    fn eapi_names() {
        assert_eq!(eapi_name.parse("8"), Ok("8"));
        assert_eq!(eapi_name.parse("_foo"), Ok("_foo"));
        assert_eq!(eapi_name.parse("Foo"), Ok("Foo"));
    }

    #[test]
    fn names() {
        assert_eq!(
            name::<_, ContextError, _, _>('A'..='Z', 'a'..='z').parse("FoO"),
            Ok("FoO")
        );
    }
}
