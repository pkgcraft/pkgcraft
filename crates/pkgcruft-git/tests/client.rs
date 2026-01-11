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
            "})
            .failure()
            .code(1);
    }
}
