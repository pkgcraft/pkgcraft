use pkgcraft::test::{cmd, TEST_DATA};

#[test]
fn args() {
    for (expr, (..)) in TEST_DATA.version_toml.compares() {
        cmd("pk version compare").arg(expr).assert().success();
    }
}
