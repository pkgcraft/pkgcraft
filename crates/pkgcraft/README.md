[![CI](https://github.com/pkgcraft/pkgcraft/workflows/CI/badge.svg)](https://github.com/pkgcraft/pkgcraft/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/pkgcraft.svg)](https://crates.io/crates/pkgcraft)

# Pkgcraft

Core library supporting various Gentoo-related functionality.

## Development

Using `cargo nextest` or another test runner that runs tests in separate
processes is required. Running tests via `cargo test` will break due to its
threaded approach since pkgcraft runs many bash-related tests and bash isn't
thread-friendly in any fashion.

For bugs or other requests please create an [issue][1].

For general support or questions use [discussions][2] or the #pkgcraft IRC
channel on libera.

[1]: <https://github.com/pkgcraft/pkgcraft/issues>
[2]: <https://github.com/pkgcraft/pkgcraft/discussions>
