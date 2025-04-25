use std::env;

use pkgcraft::test::{cmd, test_data};
use predicates::prelude::*;

super::cmd_arg_tests!("pk pkg env");

#[test]
fn ignore() {
    let data = test_data();
    let repo = data.ebuild_repo("qa-primary").unwrap();

    // invalid pkgs log errors and cause failure by default
    cmd("pk pkg env")
        .arg(repo)
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(1);

    // ignoring invalid pkgs entirely skips them
    for opt in ["-i", "--ignore"] {
        cmd("pk pkg env")
            .arg(opt)
            .arg(repo)
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();
    }
}

#[test]
fn current_dir() {
    let data = test_data();
    let repo = data.ebuild_repo("metadata").unwrap();

    // package directory
    env::set_current_dir(repo.path().join("inherit/indirect")).unwrap();
    cmd("pk pkg env -f=-FILESDIR")
        .assert()
        .stdout(indoc::indoc! {"
            BDEPEND=cat/pkg ebuild/pkg cat/pkg eclass/pkg a/pkg cat/pkg eclass/pkg b/pkg
            CATEGORY=inherit
            DEPEND=cat/pkg ebuild/pkg cat/pkg eclass/pkg a/pkg cat/pkg eclass/pkg b/pkg
            DESCRIPTION=ebuild with indirect eclass inherit
            DISTDIR=/tmp
            EAPI=8
            EPREFIX=
            HOME=/tmp
            HOMEPAGE=https://github.com/pkgcraft/a https://github.com/pkgcraft
            IDEPEND=cat/pkg ebuild/pkg cat/pkg eclass/pkg a/pkg cat/pkg eclass/pkg b/pkg
            INHERITED=b a
            IUSE=global eclass a global eclass b
            LICENSE=l1
            P=indirect-8
            PDEPEND=cat/pkg ebuild/pkg cat/pkg eclass/pkg a/pkg cat/pkg eclass/pkg b/pkg
            PF=indirect-8
            PN=indirect
            PR=r0
            PROPERTIES=global eclass a global eclass b
            PV=8
            PVR=8
            RDEPEND=cat/pkg ebuild/pkg cat/pkg eclass/pkg a/pkg cat/pkg eclass/pkg b/pkg
            REQUIRED_USE=global eclass a global eclass b
            RESTRICT=global eclass a global eclass b
            S=/tmp
            SLOT=0/1
            SRC_URI=https://github.com/pkgcraft/a.tar.xz https://github.com/pkgcraft/pkgcraft-9999.tar.xz
            T=/tmp
            TMPDIR=/tmp
            USE=
            WORKDIR=/tmp
        "})
        .stderr("")
        .success();

    // category directory
    env::set_current_dir(repo.path().join("inherit")).unwrap();
    cmd("pk pkg env -f=-FILESDIR")
        .assert()
        .stdout(indoc::indoc! {"
            inherit/direct-8::metadata
            BDEPEND=cat/pkg ebuild/pkg cat/pkg eclass/pkg a/pkg
            CATEGORY=inherit
            DEPEND=cat/pkg ebuild/pkg cat/pkg eclass/pkg a/pkg
            DESCRIPTION=ebuild with direct eclass inherit
            DISTDIR=/tmp
            EAPI=8
            EPREFIX=
            HOME=/tmp
            HOMEPAGE=https://github.com/pkgcraft
            IDEPEND=cat/pkg ebuild/pkg cat/pkg eclass/pkg a/pkg
            INHERITED=a
            IUSE=global eclass a
            LICENSE=l1
            P=direct-8
            PDEPEND=cat/pkg ebuild/pkg cat/pkg eclass/pkg a/pkg
            PF=direct-8
            PN=direct
            PR=r0
            PROPERTIES=global eclass a
            PV=8
            PVR=8
            RDEPEND=cat/pkg ebuild/pkg cat/pkg eclass/pkg a/pkg
            REQUIRED_USE=global eclass a
            RESTRICT=global eclass a
            S=/tmp
            SLOT=0
            SRC_URI=https://github.com/pkgcraft/pkgcraft-9999.tar.xz
            T=/tmp
            TMPDIR=/tmp
            USE=
            WORKDIR=/tmp

            inherit/indirect-8::metadata
            BDEPEND=cat/pkg ebuild/pkg cat/pkg eclass/pkg a/pkg cat/pkg eclass/pkg b/pkg
            CATEGORY=inherit
            DEPEND=cat/pkg ebuild/pkg cat/pkg eclass/pkg a/pkg cat/pkg eclass/pkg b/pkg
            DESCRIPTION=ebuild with indirect eclass inherit
            DISTDIR=/tmp
            EAPI=8
            EPREFIX=
            HOME=/tmp
            HOMEPAGE=https://github.com/pkgcraft/a https://github.com/pkgcraft
            IDEPEND=cat/pkg ebuild/pkg cat/pkg eclass/pkg a/pkg cat/pkg eclass/pkg b/pkg
            INHERITED=b a
            IUSE=global eclass a global eclass b
            LICENSE=l1
            P=indirect-8
            PDEPEND=cat/pkg ebuild/pkg cat/pkg eclass/pkg a/pkg cat/pkg eclass/pkg b/pkg
            PF=indirect-8
            PN=indirect
            PR=r0
            PROPERTIES=global eclass a global eclass b
            PV=8
            PVR=8
            RDEPEND=cat/pkg ebuild/pkg cat/pkg eclass/pkg a/pkg cat/pkg eclass/pkg b/pkg
            REQUIRED_USE=global eclass a global eclass b
            RESTRICT=global eclass a global eclass b
            S=/tmp
            SLOT=0/1
            SRC_URI=https://github.com/pkgcraft/a.tar.xz https://github.com/pkgcraft/pkgcraft-9999.tar.xz
            T=/tmp
            TMPDIR=/tmp
            USE=
            WORKDIR=/tmp

            inherit/none-8::metadata
            BDEPEND=cat/pkg ebuild/pkg
            CATEGORY=inherit
            DEPEND=cat/pkg ebuild/pkg
            DESCRIPTION=ebuild with no eclass inherits
            DISTDIR=/tmp
            EAPI=8
            EPREFIX=
            HOME=/tmp
            HOMEPAGE=https://github.com/pkgcraft
            IDEPEND=cat/pkg ebuild/pkg
            LICENSE=l1
            P=none-8
            PDEPEND=cat/pkg ebuild/pkg
            PF=none-8
            PN=none
            PR=r0
            PV=8
            PVR=8
            RDEPEND=cat/pkg ebuild/pkg
            S=/tmp
            SLOT=0
            SRC_URI=https://github.com/pkgcraft/pkgcraft-9999.tar.xz
            T=/tmp
            TMPDIR=/tmp
            USE=
            WORKDIR=/tmp
        "})
        .stderr("")
        .success();
}
