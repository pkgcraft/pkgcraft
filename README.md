[![CI](https://github.com/pkgcraft/pkgcraft/workflows/CI/badge.svg)](https://github.com/pkgcraft/pkgcraft/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/pkgcraft.svg)](https://crates.io/crates/pkgcraft)

# Pkgcraft

Pkgcraft is a highly experimental, rust-based, tooling ecosystem for Gentoo. It
aims to provide bindings for other programming languages targeting
Gentoo-specific functionality as well as a new approach to package management,
leveraging a client-server design that will potentially support various
frontends.

## Components

- **pkgcraft**: core library supporting various Gentoo-related functionality
- **arcanist**: daemon focused on package querying, building, and merging
- **pakt**: command-line client for arcanist

## Development

Note that using `cargo nextest` or another test runner that runs tests in
separate processes is required. Using `cargo test` will break as long as it
uses threads since pkgcraft runs many bash-related tests and bash isn't
thread-friendly in any fashion.

For bugs and feature requests please create an [issue][1].

Otherwise [discussions][2] can be used for general questions and support.

[1]: <https://github.com/pkgcraft/pkgcraft/issues>
[2]: <https://github.com/pkgcraft/pkgcraft/discussions>
