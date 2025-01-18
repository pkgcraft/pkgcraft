QA library and tools based on pkgcraft.

# Usage

The main use for pkgcruft is to scan ebuild repos for issues. It works in a
similar fashion to [pkgcheck] but operates in a much more performant manner
while supporting features such as ignore directives, native package filtering,
and sorted output.

For basic use cases, simply run `pkgcruft scan` inside an ebuild repo.

# Build requirements

A recent version of rust and compatible clang compiler.

# Install

Build and install from crates.io:

    cargo install pkgcruft

Build and install from git:

    cargo install pkgcruft --git https://github.com/pkgcraft/pkgcraft.git

Install with cargo-binstall:

    cargo binstall pkgcruft

# Benchmarking

Benchmarks can be run individually for every supported check against a repo target:

    # set repo target
    export PKGCRUFT_BENCH_REPO=path/to/repo

    # run benchmarks
    cargo criterion Check --features test -p pkgcruft

[pkgcheck]: <https://github.com/pkgcore/pkgcheck/>
