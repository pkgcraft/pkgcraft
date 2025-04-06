use itertools::Itertools;
use pkgcraft::test::{cmd, test_data};

use crate::predicates::lines_contain;

#[test]
fn stdin() {
    let data = test_data();
    let exprs = data.version_toml.compares().map(|(s, _)| s).join("\n");
    cmd("pk version compare -")
        .write_stdin(exprs)
        .assert()
        .success();
}

#[test]
fn args() {
    // invalid expression
    cmd("pk version compare")
        .arg("1<2")
        .assert()
        .stdout("")
        .stderr(lines_contain(["invalid comparison format: 1<2"]))
        .failure()
        .code(2);

    // invalid operator
    for op in ["~=", "=", "+="] {
        cmd("pk version compare")
            .arg(format!("1 {op} 2"))
            .assert()
            .stdout("")
            .stderr(lines_contain([format!("invalid operator: 1 {op} 2")]))
            .failure()
            .code(2);
    }

    // invalid version
    cmd("pk version compare")
        .arg("2 >= a1")
        .assert()
        .stdout("")
        .stderr(lines_contain(["invalid version: a1"]))
        .failure()
        .code(2);

    // comparing versions with and without operators
    for expr in ["1 < <1", "1 != =1"] {
        cmd("pk version compare").arg(expr).assert().success();
    }

    // false expressions
    for expr in ["1 > 2", "<1 > >2", "1.1 == 1.10"] {
        cmd("pk version compare")
            .arg(expr)
            .assert()
            .failure()
            .code(1);
    }

    // true expressions
    for expr in ["1 <= 2", "<1 <= >2", "1.1 != 1.01"] {
        cmd("pk version compare").arg(expr).assert().success();
    }
}
