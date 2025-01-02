[![crates.io](https://img.shields.io/crates/v/scallop.svg)](https://crates.io/crates/scallop)

# scallop

Scallop wraps a forked version of [bash] supporting shell interactions (e.g.
writing builtins or modifying variables, arrays, and functions) natively in
rust.

## Development

Note that currently the development workflow involves force pushing to the
[bash] repo in order to keep the patch stack in order when upstream changes are
merged.

[bash]: <https://github.com/pkgcraft/bash>
