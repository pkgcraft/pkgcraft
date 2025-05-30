Client and server for verifying git commits via pkgcruft during server-side pre-receive hook.

# Demo

To run an interactive service demo, run the examples/pkgcruft-gitd script:

    ./examples/pkgcruft-gitd

By default it uses the gentoo repo from github, but should work if pointed at
an accessible, remote git URL for a standalone ebuild repo.

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
