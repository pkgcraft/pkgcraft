[![ci](https://github.com/pkgcraft/pkgcraft/workflows/ci/badge.svg)](https://github.com/pkgcraft/pkgcraft/actions/workflows/ci.yml)
[![coverage](https://codecov.io/gh/pkgcraft/pkgcraft/branch/main/graph/badge.svg)](https://codecov.io/gh/pkgcraft/pkgcraft)

# Pkgcraft

Highly experimental tooling ecosystem for Gentoo comprised of the following:

- [scallop]: bash library
- [pkgcraft]: core library
- [pkgcraft-c]: C bindings
- [pkgcraft-tools]: command-line tools
- [pkgcruft]: QA library and tools
- [arcanist]: package-building daemon

More information about the project can be found on its [FAQ] and
[development blog][website].

## Development

Using [cargo-nextest] is required to run tests in separate processes. Running
tests via `cargo test` will break due to its threaded approach since much of
the pkgcraft ecosystem relies on bash which isn't thread-friendly in any
fashion.

In addition, crates with the `test` feature require it to be enabled when
running tests so use `cargo nextest run --all-features --tests` to run tests
for the entire workspace.

For bugs or other requests use [issues].

For general support or questions use [discussions].

[faq]: <https://pkgcraft.github.io/about/>
[website]: <https://pkgcraft.github.io/>
[cargo-nextest]: <https://nexte.st/>
[issues]: <https://github.com/pkgcraft/pkgcraft/issues>
[discussions]: <https://github.com/pkgcraft/pkgcraft/discussions>

[scallop]: <https://github.com/pkgcraft/pkgcraft/tree/main/crates/scallop>
[pkgcraft]: <https://github.com/pkgcraft/pkgcraft/tree/main/crates/pkgcraft>
[pkgcraft-c]: <https://github.com/pkgcraft/pkgcraft/tree/main/crates/pkgcraft-c>
[pkgcraft-tools]: <https://github.com/pkgcraft/pkgcraft/tree/main/crates/pkgcraft-tools>
[pkgcruft]: <https://github.com/pkgcraft/pkgcraft/tree/main/crates/pkgcruft>
[arcanist]: <https://github.com/pkgcraft/pkgcraft/tree/main/crates/arcanist>
