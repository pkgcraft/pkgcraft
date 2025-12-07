[![crates.io](https://img.shields.io/crates/v/scallop.svg)](https://crates.io/crates/scallop)

# scallop

Scallop wraps a forked version of [bash] supporting shell interactions (e.g.
writing builtins or modifying variables, arrays, and functions) natively in
rust.

By default, a vendored copy of bash is built and linked into binaries as a
static library. Using a shared library is possible by exporting
`SCALLOP_NO_VENDOR=1` during the build process which uses pkg-config to verify
the library version matches the vendored version as bash doesn't use semantic
versioning (mostly because upstream doesn't support building as a library).

[bash]: <https://github.com/pkgcraft/bash>
