use pkgcraft::repo::ebuild_temp::Repo as TempRepo;
use pkgcraft::test::cmd;

#[test]
fn single_leaf_pkg() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_ebuild("cat/dep-1", &[]).unwrap();
    t.create_ebuild("cat/leaf-1", &["DEPEND=>=cat/dep-1"])
        .unwrap();
    cmd(&format!("pk repo leaf {}", t.path()))
        .assert()
        .stdout("cat/leaf-1\n");
}
