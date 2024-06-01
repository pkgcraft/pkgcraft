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

Benchmarks can be run individually for every check against a specified external
repo:

    # set the repo to target for benchmarking
    export PKGCRUFT_BENCH_REPO=path/to/repo

    # make sure the repo's metadata cache is up to date
    pk repo metadata regen path/to/repo

    # run the benchmarks from the root directory of the pkgcraft git repo
    cargo criterion Check --features test -p pkgcruft
