QA library and tools based on pkgcraft.

# Requirements

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
