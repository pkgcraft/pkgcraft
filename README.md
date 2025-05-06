[![ci](https://github.com/pkgcraft/pkgcraft/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/pkgcraft/pkgcraft/actions/workflows/ci.yml)
[![coverage](https://codecov.io/gh/pkgcraft/pkgcraft/branch/main/graph/badge.svg)](https://codecov.io/gh/pkgcraft/pkgcraft)

# Pkgcraft

Highly experimental tooling ecosystem for Gentoo comprised of the following:

- [scallop]: bash library
- [pkgcraft]: core library
- [pkgcraft-c]: C bindings
- [pkgcraft-tools]: command-line tools
- [pkgcruft]: QA library and tools
- [arcanist]: package-building daemon

Compatibility with the official [package management specification][pmspec] is
aimed for, but not always adhered to. See the list of known [deviations] for
details.

Repos using custom profile-formats extensions are not supported for tools that
deal with profiles, see the [related issue][profile-formats] for details.

More information about the project can be found on its [FAQ] and
[development blog][blog].

## Development

Using [cargo-nextest] is required to run tests in separate processes. Running
tests via `cargo test` will break due to its threaded approach since much of
the pkgcraft ecosystem relies on bash which isn't thread-friendly in any
fashion.

To run tests across all crates use: `cargo nextest run --all-features --tests`

For bugs or other requests use [issues].

For general support or questions use [discussions].

[faq]: <https://pkgcraft.github.io/about/>
[blog]: <https://pkgcraft.github.io/>
[cargo-nextest]: <https://nexte.st/>
[issues]: <https://github.com/pkgcraft/pkgcraft/issues>
[discussions]: <https://github.com/pkgcraft/pkgcraft/discussions>
[pmspec]: https://wiki.gentoo.org/wiki/Project:Package_Manager_Specification
[deviations]: https://github.com/orgs/pkgcraft/discussions/134
[profile-formats]: https://github.com/pkgcraft/pkgcraft/issues/251

[scallop]: <https://github.com/pkgcraft/pkgcraft/tree/main/crates/scallop>
[pkgcraft]: <https://github.com/pkgcraft/pkgcraft/tree/main/crates/pkgcraft>
[pkgcraft-c]: <https://github.com/pkgcraft/pkgcraft/tree/main/crates/pkgcraft-c>
[pkgcraft-tools]: <https://github.com/pkgcraft/pkgcraft/tree/main/crates/pkgcraft-tools>
[pkgcruft]: <https://github.com/pkgcraft/pkgcraft/tree/main/crates/pkgcruft>
[arcanist]: <https://github.com/pkgcraft/pkgcraft/tree/main/crates/arcanist>
