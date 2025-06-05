# pkgcruft-git

QA support for verifying git commits via pkgcruft.

## Install

Build and install from git:

    cargo install pkgcruft-git --git https://github.com/pkgcraft/pkgcraft.git

## Client side commit verification via git pre-push hook

To trigger commit verification before pushing to a remote, symlink the
pkgcruft-git-pre-push binary as a pre-push hook for the target git repo:

    cd path/to/git/repo
    mkdir -p .git/hooks
    ln -s $(which pkgcruft-git-pre-push) .git/hooks/pre-push

When using multiple pre-push hooks, it will have to be called manually passing
the expected remote arguments.

## Server side commit verification via git pre-receive hook

To start a service demo, run the following script:

    ./examples/pkgcruft-gitd

This will build the required pkgcraft tooling and initialize the files
necessary to run the service in the `examples/demo` directory. By default it
uses the gentoo repo from github, but should work if pointed at any accessible,
remote git URL for a standalone ebuild repo.

The script creates three git repos under the `examples/demo` directory. The
`client.git` repo is what users should interact with, creating commits and
using `git push` that pushes them to the `remote.git` repo. The remote repo is
configured to trigger pkgcruft scanning runs via a pre-receive hook targeting
the changes made in the commits.

Once the repos are set up, the script ends by starting the pkgcruft-gitd
service that runs against the `server.git` repo. The service can be stopped via
SIGINT and restarted by re-execing the script.

To reset the demo entirely, recursively remove the examples/demo directory
before running the script.
