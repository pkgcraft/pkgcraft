Client and server for verifying git commits via pkgcruft during server-side pre-receive hook.

To set up an interactive demo, run the examples/pkgcruft-gitd script. By
default it uses the gentoo repo from github, but should work if pointed at a
remote git URL for any standalone ebuild repo. Once the service is running,
users can create commits in the git repo at examples/demo/client.git and push
them to the origin remote to trigger scanning via pre-receive hook. To reset
the demo, recursively remove the examples/demo directory.
