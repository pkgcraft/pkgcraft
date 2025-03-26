use scallop::{Error, ExecStatus};

use super::make_builtin;

static LONG_DOC: &str = "Error out on direct phase function calls";

#[doc = stringify!(LONG_DOC)]
fn run(_args: &[&str]) -> scallop::Result<ExecStatus> {
    Err(Error::Base("direct phase call".to_string()))
}

make_builtin!("pkg_config", pkg_config, PKG_CONFIG, false);
make_builtin!("pkg_info", pkg_info, PKG_INFO, false);
make_builtin!("pkg_nofetch", pkg_nofetch, PKG_NOFETCH, false);
make_builtin!("pkg_postinst", pkg_postinst, PKG_POSTINST, false);
make_builtin!("pkg_postrm", pkg_postrm, PKG_POSTRM, false);
make_builtin!("pkg_preinst", pkg_preinst, PKG_PREINST, false);
make_builtin!("pkg_prerm", pkg_prerm, PKG_PRERM, false);
make_builtin!("pkg_pretend", pkg_pretend, PKG_PRETEND, false);
make_builtin!("pkg_setup", pkg_setup, PKG_SETUP, false);
make_builtin!("src_compile", src_compile, SRC_COMPILE, false);
make_builtin!("src_configure", src_configure, SRC_CONFIGURE, false);
make_builtin!("src_install", src_install, SRC_INSTALL, false);
make_builtin!("src_prepare", src_prepare, SRC_PREPARE, false);
make_builtin!("src_test", src_test, SRC_TEST, false);
make_builtin!("src_unpack", src_unpack, SRC_UNPACK, false);
