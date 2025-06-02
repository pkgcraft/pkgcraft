# pkgcruft-git

Tools and services for verifying git commits via pkgcruft.

## Server side commit verification via git pre-receive hook

To start an interactive service demo, run the following script:

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
