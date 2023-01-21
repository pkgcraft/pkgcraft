[![ci](https://github.com/pkgcraft/pkgcraft/workflows/ci/badge.svg)](https://github.com/pkgcraft/pkgcraft/actions/workflows/ci.yml)

# Pkgcraft

Highly experimental tooling ecosystem for Gentoo comprised of the following:

- scallop: bash support
- pkgcraft: Gentoo functionality 
- pkgcraft-c: C library for language bindings
- arcanist: package-building daemon

## Development

Using `cargo nextest` is required to run tests in separate processes. Running
tests via `cargo test` will break due to its threaded approach since much of
the pkgcraft ecosystem relies on bash which isn't thread-friendly in any
fashion.

For bugs or other requests please create an [issue][1].

For general support or questions use [discussions][2] or the #pkgcraft IRC
channel on libera.

[1]: <https://github.com/pkgcraft/pkgcraft/issues>
[2]: <https://github.com/pkgcraft/pkgcraft/discussions>
