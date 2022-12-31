# scallop

Scallop is a rust-based library that wraps bash. It supports writing bash
builtins in rust and interacting with various bash data structures including
variables, arrays, and functions.

## Development

Developing scallop requires recent versions of rust, cargo, and [cargo-nextest](https://nexte.st/) are installed
along with a standard C compiler.

Testing requires cargo-nextest since it runs tests in separate processes as
opposed to `cargo test` that uses threads which breaks since bash isn't
thread-friendly in any fashion.

To build scallop, run the following commands:

```bash
git clone --recurse-submodules https://github.com/pkgcraft/scallop.git
cd scallop

# build and run tests
cargo nextest run
```
