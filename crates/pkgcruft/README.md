QA library and tools based on pkgcraft.

# WARNING

Pkgcraft currently lacks proper handling for generating ebuild metadata in
threads so pkgcruft will often crash when run on repos lacking metadata (see
issue #178).

As a workaround, the command `pk pkg metadata` can be called from any ebuild
repo directory to generate related package metadata and on successful
completion `pkgcruft scan` can be called.

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
