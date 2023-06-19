use pkgcraft::test::{cmd, TEST_DATA};

#[test]
fn args() {
    for (expr, (..)) in TEST_DATA.dep_toml.compares() {
        cmd("pk dep compare").arg(expr).assert().success();
    }

    for (_, (s1, op, s2)) in TEST_DATA.version_toml.compares() {
        cmd("pk dep compare")
            .arg(format!("=cat/pkg-{s1} {op} =cat/pkg-{s2}"))
            .assert()
            .success();
    }
}
