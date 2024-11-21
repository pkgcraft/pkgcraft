QA library and tools based on pkgcraft.

# Requirements

A recent version of rust and compatible clang compiler.

# Install

Static binaries are available for releases on supported platforms or `cargo
install` can be used.

Install from crates.io:

    cargo install pkgcruft

Install from git:

    cargo install pkgcruft --git https://github.com/pkgcraft/pkgcraft.git

# Benchmarking

Benchmarks can be run individually for every check against a repo target:

    # set repo target
    export PKGCRUFT_BENCH_REPO=path/to/repo

    # update repo metadata
    pk repo metadata regen path/to/repo

    # run benchmarks
    cargo criterion Check --features test -p pkgcruft
