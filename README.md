[![ci](https://github.com/pkgcraft/pkgcraft/workflows/ci/badge.svg)](https://github.com/pkgcraft/pkgcraft/actions/workflows/ci.yml)
[![coverage](https://codecov.io/gh/pkgcraft/pkgcraft/branch/main/graph/badge.svg)](https://codecov.io/gh/pkgcraft/pkgcraft)

# Pkgcraft

Highly experimental tooling ecosystem for Gentoo comprised of the following:

- scallop: bash library
- pkgcraft: core library
- pkgcraft-c: C bindings
- pkgcraft-tools: command-line tools
- pkgcruft: QA library and tools
- arcanist: package-building daemon

More information about the project can be found on its [FAQ][0] and
[development blog][1].

## Development

Using [cargo-nextest][2] is required to run tests in separate processes. Running
tests via `cargo test` will break due to its threaded approach since much of
the pkgcraft ecosystem relies on bash which isn't thread-friendly in any
fashion.

In addition, crates with the `test` feature require it to be enabled when
running tests so use `cargo nextest run --all-features --tests` to run tests
for the entire workspace.

For bugs or other requests please create an [issue][3].

For general support or questions use [discussions][4] or the #pkgcraft IRC
channel on libera.

[0]: <https://pkgcraft.github.io/about/>
[1]: <https://pkgcraft.github.io/>
[2]: <https://nexte.st/>
[3]: <https://github.com/pkgcraft/pkgcraft/issues>
[4]: <https://github.com/pkgcraft/pkgcraft/discussions>
