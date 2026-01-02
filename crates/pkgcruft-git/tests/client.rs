use crate::cmd;

#[test]
fn connect() {
    for opt in ["-c", "--connect"] {
        // nonexistent socket
        let socket = "/path/to/nonexistent/socket";
        cmd("pkgcruft-git")
            .args([opt, socket])
            .arg("version")
            .assert()
            .stdout("")
            .stderr(indoc::formatdoc! {"
                pkgcruft-git: error: failed connecting to service: {socket}

                caused by: transport error
                caused by: No such file or directory (os error 2)
                caused by: No such file or directory (os error 2)
            "})
            .failure()
            .code(1);
    }
}
