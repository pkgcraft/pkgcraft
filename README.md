[![ci](https://github.com/pkgcraft/pkgcraft/workflows/ci/badge.svg)](https://github.com/pkgcraft/pkgcraft/actions/workflows/ci.yml)
[![coverage](https://codecov.io/gh/pkgcraft/pkgcraft/branch/main/graph/badge.svg)](https://codecov.io/gh/pkgcraft/pkgcraft)

# Pkgcraft

Highly experimental tooling ecosystem for Gentoo comprised of the following:

- scallop: bash library
- pkgcraft: core library
- pkgcraft-c: C bindings
- pkgcraft-tools: command-line tools
- arcanist: package-building daemon

## Development

Using [cargo-nextest][0] is required to run tests in separate processes. Running
tests via `cargo test` will break due to its threaded approach since much of
the pkgcraft ecosystem relies on bash which isn't thread-friendly in any
fashion.

In addition, crates with the `test` feature require it to be enabled when
running tests so use `cargo nextest run --all-features --tests` to run tests
for the entire workspace.

For bugs or other requests please create an [issue][1].

For general support or questions use [discussions][2] or the #pkgcraft IRC
channel on libera.

[0]: <https://nexte.st/>
[1]: <https://github.com/pkgcraft/pkgcraft/issues>
[2]: <https://github.com/pkgcraft/pkgcraft/discussions>
