use std::fs::File;
use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use scallop::{Error, Result};

use super::RunCommand;
use crate::eapi::{Eapi, EAPI_LATEST};

pub(super) trait Compression {
    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(src: P, dest: Q) -> Result<()>;
    fn unpack<P: AsRef<Utf8Path>>(&self, dest: P) -> Result<()>;
}

#[derive(Debug)]
pub(super) struct Tar {
    path: Utf8PathBuf,
}

impl Compression for Tar {
    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(src: P, dest: Q) -> Result<()> {
        let src = src.as_ref();
        let dest = dest.as_ref();
        let mut cmd = Command::new("tar");
        cmd.args(["-c", src.as_str(), "-f", dest.as_str()]);
        cmd.run()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> Result<()> {
        let mut cmd = Command::new("tar");
        cmd.arg("xf").arg(&self.path);
        cmd.run()
    }
}

#[derive(Debug)]
pub(super) struct TarGz {
    path: Utf8PathBuf,
}

impl Compression for TarGz {
    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(src: P, dest: Q) -> Result<()> {
        let src = src.as_ref();
        let dest = dest.as_ref();
        let mut cmd = Command::new("tar");
        cmd.args(["-c", src.as_str(), "-I", "gzip", "-f", dest.as_str()]);
        cmd.run()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> Result<()> {
        let mut cmd = Command::new("tar");
        cmd.args(["xf", self.path.as_str(), "-I", "gzip"]);
        cmd.run()
    }
}

#[derive(Debug)]
pub(super) struct TarBz2 {
    path: Utf8PathBuf,
}

impl Compression for TarBz2 {
    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(src: P, dest: Q) -> Result<()> {
        let src = src.as_ref();
        let dest = dest.as_ref();
        let mut cmd = Command::new("tar");
        cmd.args(["-c", src.as_str(), "-I", "bzip2", "-f", dest.as_str()]);
        cmd.run()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> Result<()> {
        let mut cmd = Command::new("tar");
        cmd.args(["xf", self.path.as_str(), "-I", "bzip2"]);
        cmd.run()
    }
}

#[derive(Debug)]
pub(super) struct TarLzma {
    path: Utf8PathBuf,
}

impl Compression for TarLzma {
    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(src: P, dest: Q) -> Result<()> {
        let src = src.as_ref();
        let dest = dest.as_ref();
        let mut cmd = Command::new("tar");
        cmd.args(["-c", src.as_str(), "-I", "lzma", "-f", dest.as_str()]);
        cmd.run()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> Result<()> {
        let mut cmd = Command::new("tar");
        cmd.args(["xf", self.path.as_str(), "-I", "lzma"]);
        cmd.run()
    }
}

#[derive(Debug)]
pub(super) struct TarXz {
    path: Utf8PathBuf,
}

impl Compression for TarXz {
    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(src: P, dest: Q) -> Result<()> {
        let src = src.as_ref();
        let dest = dest.as_ref();
        let mut cmd = Command::new("tar");
        cmd.args(["-c", src.as_str(), "-I", "xz", "-f", dest.as_str()]);
        cmd.run()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> Result<()> {
        let mut cmd = Command::new("tar");
        cmd.args(["xf", self.path.as_str(), "-I", "xz"]);
        cmd.run()
    }
}

#[derive(Debug)]
pub(super) struct Zip {
    path: Utf8PathBuf,
}

impl Compression for Zip {
    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(_src: P, _dest: Q) -> Result<()> {
        unimplemented!()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> Result<()> {
        let mut cmd = Command::new("unzip");
        cmd.args(["-qo", self.path.as_str()]);
        cmd.run()
    }
}

#[derive(Debug)]
pub(super) struct Gz {
    path: Utf8PathBuf,
}

impl Compression for Gz {
    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(_src: P, _dest: Q) -> Result<()> {
        unimplemented!()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, dest: P) -> Result<()> {
        let src = &self.path;
        let src = File::open(src)
            .map_err(|e| Error::Base(format!("failed reading archive: {src:?}: {e}")))?;

        let dest = dest.as_ref();
        let dest = File::create(dest)
            .map_err(|e| Error::Base(format!("failed creating archive: {dest:?}: {e}")))?;

        let mut cmd = Command::new("gzip");
        cmd.arg("-d").arg("-c").stdin(src).stdout(dest);
        cmd.run()
    }
}

#[derive(Debug)]
pub(super) struct Bz2 {
    path: Utf8PathBuf,
}

impl Compression for Bz2 {
    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(_src: P, _dest: Q) -> Result<()> {
        unimplemented!()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, dest: P) -> Result<()> {
        let src = &self.path;
        let src = File::open(src)
            .map_err(|e| Error::Base(format!("failed reading archive: {src:?}: {e}")))?;

        let dest = dest.as_ref();
        let dest = File::create(dest)
            .map_err(|e| Error::Base(format!("failed creating archive: {dest:?}: {e}")))?;

        let mut cmd = Command::new("bzip2");
        cmd.arg("-d").arg("-c").stdin(src).stdout(dest);
        cmd.run()
    }
}

#[derive(Debug)]
pub(super) struct Xz {
    path: Utf8PathBuf,
}

impl Compression for Xz {
    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(_src: P, _dest: Q) -> Result<()> {
        unimplemented!()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, dest: P) -> Result<()> {
        let src = &self.path;
        let src = File::open(src)
            .map_err(|e| Error::Base(format!("failed reading archive: {src:?}: {e}")))?;

        let dest = dest.as_ref();
        let dest = File::create(dest)
            .map_err(|e| Error::Base(format!("failed creating archive: {dest:?}: {e}")))?;

        let mut cmd = Command::new("xz");
        cmd.arg("-d").arg("-c").stdin(src).stdout(dest);
        cmd.run()
    }
}

#[derive(Debug)]
pub(super) struct _7z {
    path: Utf8PathBuf,
}

impl Compression for _7z {
    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(_src: P, _dest: Q) -> Result<()> {
        unimplemented!()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> Result<()> {
        let mut cmd = Command::new("7z");
        cmd.args(["x", "-y", self.path.as_str()]);
        cmd.run()
    }
}

#[derive(Debug)]
pub(super) struct Rar {
    path: Utf8PathBuf,
}

impl Compression for Rar {
    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(_src: P, _dest: Q) -> Result<()> {
        unimplemented!()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> Result<()> {
        let mut cmd = Command::new("unrar");
        cmd.args(["x", "-idq", "-o+", self.path.as_str()]);
        cmd.run()
    }
}

#[derive(Debug)]
pub(super) struct Lha {
    path: Utf8PathBuf,
}

impl Compression for Lha {
    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(_src: P, _dest: Q) -> Result<()> {
        unimplemented!()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> Result<()> {
        let mut cmd = Command::new("lha");
        cmd.args(["xfq", self.path.as_str()]);
        cmd.run()
    }
}

#[derive(Debug)]
pub(super) struct Ar {
    path: Utf8PathBuf,
}

impl Compression for Ar {
    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(_src: P, _dest: Q) -> Result<()> {
        unimplemented!()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> Result<()> {
        let mut cmd = Command::new("ar");
        cmd.args(["x", self.path.as_str()]);
        cmd.run()
    }
}

#[derive(Debug)]
pub(super) struct Lzma {
    path: Utf8PathBuf,
}

impl Compression for Lzma {
    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(_src: P, _dest: Q) -> Result<()> {
        unimplemented!()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, dest: P) -> Result<()> {
        let dest = dest.as_ref();
        let dest = File::create(dest)
            .map_err(|e| Error::Base(format!("failed creating archive: {dest:?}: {e}")))?;

        let mut cmd = Command::new("lzma");
        cmd.arg("-dc").arg(&self.path).stdout(dest);
        cmd.run()
    }
}

macro_rules! make_archive {
    ($($x:ident),*) => {
        #[derive(Debug)]
        pub(super) enum Archive {
            $(
                $x($x),
            )*
        }

        impl Compression for Archive {
            fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(src: P, dest: Q) -> Result<()> {
                let (_, archive) = Archive::from_path(dest.as_ref(), EAPI_LATEST)?;
                match archive {
                    $(
                        Archive::$x(_) => $x::pack(src, dest),
                    )*
                }
            }

            fn unpack<P: AsRef<Utf8Path>>(&self, dest: P) -> Result<()> {
                match self {
                    $(
                        Archive::$x(a) => a.unpack(dest),
                    )*
                }
            }
        }
    };
}
make_archive!(Tar, TarGz, TarBz2, TarLzma, TarXz, Zip, Gz, Bz2, Xz, _7z, Rar, Lha, Ar, Lzma);

impl Archive {
    pub(super) fn from_path<P: AsRef<Utf8Path>>(
        path: P,
        eapi: &Eapi,
    ) -> crate::Result<(String, Archive)> {
        let path = path.as_ref();

        let mut ext = match eapi.archives_regex().captures(path.as_str()) {
            Some(c) => String::from(c.name("ext").unwrap().as_str()),
            None => String::from(""),
        };

        if eapi.has("unpack_case_insensitive") {
            ext = ext.to_lowercase();
        }

        let path = Utf8PathBuf::from(path);

        match ext.as_str() {
            "tar" => Ok((ext, Archive::Tar(Tar { path }))),
            "tar.gz" | "tgz" | "tar.z" | "tar.Z" => Ok((ext, Archive::TarGz(TarGz { path }))),
            "tar.bz2" | "tbz2" | "tbz" => Ok((ext, Archive::TarBz2(TarBz2 { path }))),
            "tar.lzma" => Ok((ext, Archive::TarLzma(TarLzma { path }))),
            "tar.xz" | "txz" => Ok((ext, Archive::TarXz(TarXz { path }))),
            "zip" | "ZIP" | "jar" => Ok((ext, Archive::Zip(Zip { path }))),
            "gz" | "z" | "Z" => Ok((ext, Archive::Gz(Gz { path }))),
            "bz2" | "bz" => Ok((ext, Archive::Bz2(Bz2 { path }))),
            "xz" => Ok((ext, Archive::Xz(Xz { path }))),
            "7z" | "7Z" => Ok((ext, Archive::_7z(_7z { path }))),
            "rar" | "RAR" => Ok((ext, Archive::Rar(Rar { path }))),
            "lha" | "LHA" | "LHa" | "lzh" => Ok((ext, Archive::Lha(Lha { path }))),
            "deb" | "a" => Ok((ext, Archive::Ar(Ar { path }))),
            "lzma" => Ok((ext, Archive::Lzma(Lzma { path }))),
            _ => Err(crate::Error::InvalidValue(format!("unknown archive format: {path:?}"))),
        }
    }
}
