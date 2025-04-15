use std::fs::File;
use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};

use crate::command::RunCommand;
use crate::Error;

pub(crate) trait ArchiveFormat {
    const EXTS: &'static [&'static str];
    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(src: P, dest: Q) -> crate::Result<()>;
    fn unpack<P: AsRef<Utf8Path>>(&self, dest: P) -> crate::Result<()>;
}

#[derive(Debug)]
pub(crate) struct Tar {
    path: Utf8PathBuf,
}

impl ArchiveFormat for Tar {
    const EXTS: &'static [&'static str] = &["tar"];

    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(src: P, dest: Q) -> crate::Result<()> {
        let src = src.as_ref();
        let dest = dest.as_ref();
        let mut cmd = Command::new("tar");
        cmd.args(["-f", dest.as_str(), "-c", src.as_str()]);
        cmd.run()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> crate::Result<()> {
        let mut cmd = Command::new("tar");
        cmd.arg("xf").arg(&self.path);
        cmd.run()
    }
}

#[derive(Debug)]
pub(crate) struct TarGz {
    path: Utf8PathBuf,
}

impl ArchiveFormat for TarGz {
    const EXTS: &'static [&'static str] = &["tar.gz", "tgz", "tar.z"];

    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(src: P, dest: Q) -> crate::Result<()> {
        let src = src.as_ref();
        let dest = dest.as_ref();
        let mut cmd = Command::new("tar");
        cmd.args(["--use-compress-program", "gzip", "-f", dest.as_str(), "-c", src.as_str()]);
        cmd.run()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> crate::Result<()> {
        let mut cmd = Command::new("tar");
        cmd.args(["xf", self.path.as_str()]);
        cmd.run()
    }
}

#[derive(Debug)]
pub(crate) struct TarBz2 {
    path: Utf8PathBuf,
}

impl ArchiveFormat for TarBz2 {
    const EXTS: &'static [&'static str] = &["tar.bz2", "tbz2", "tbz"];

    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(src: P, dest: Q) -> crate::Result<()> {
        let src = src.as_ref();
        let dest = dest.as_ref();
        let mut cmd = Command::new("tar");
        cmd.args(["--use-compress-program", "bzip2", "-f", dest.as_str(), "-c", src.as_str()]);
        cmd.run()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> crate::Result<()> {
        let mut cmd = Command::new("tar");
        cmd.args(["xf", self.path.as_str()]);
        cmd.run()
    }
}

#[derive(Debug)]
pub(crate) struct TarLzma {
    path: Utf8PathBuf,
}

impl ArchiveFormat for TarLzma {
    const EXTS: &'static [&'static str] = &["tar.lzma"];

    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(src: P, dest: Q) -> crate::Result<()> {
        let src = src.as_ref();
        let dest = dest.as_ref();
        let mut cmd = Command::new("tar");
        cmd.args(["--use-compress-program", "lzma", "-f", dest.as_str(), "-c", src.as_str()]);
        cmd.run()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> crate::Result<()> {
        let mut cmd = Command::new("tar");
        cmd.args(["xf", self.path.as_str()]);
        cmd.run()
    }
}

#[derive(Debug)]
pub(crate) struct TarXz {
    path: Utf8PathBuf,
}

impl ArchiveFormat for TarXz {
    const EXTS: &'static [&'static str] = &["tar.xz", "txz"];

    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(src: P, dest: Q) -> crate::Result<()> {
        let src = src.as_ref();
        let dest = dest.as_ref();
        let mut cmd = Command::new("tar");
        cmd.args(["--use-compress-program", "xz", "-f", dest.as_str(), "-c", src.as_str()]);
        cmd.run()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> crate::Result<()> {
        let mut cmd = Command::new("tar");
        cmd.args(["xf", self.path.as_str()]);
        cmd.run()
    }
}

#[derive(Debug)]
pub(crate) struct Zip {
    path: Utf8PathBuf,
}

impl ArchiveFormat for Zip {
    const EXTS: &'static [&'static str] = &["zip", "jar"];

    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(_src: P, _dest: Q) -> crate::Result<()> {
        unimplemented!()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> crate::Result<()> {
        let mut cmd = Command::new("unzip");
        cmd.args(["-qo", self.path.as_str()]);
        cmd.run()
    }
}

#[derive(Debug)]
pub(crate) struct Gz {
    path: Utf8PathBuf,
}

impl ArchiveFormat for Gz {
    const EXTS: &'static [&'static str] = &["gz", "z"];

    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(src: P, dest: Q) -> crate::Result<()> {
        let src = src.as_ref();
        let src = File::open(src)
            .map_err(|e| Error::IO(format!("failed reading file: {src}: {e}")))?;

        let dest = dest.as_ref();
        let dest = File::create(dest)
            .map_err(|e| Error::IO(format!("failed creating file: {dest}: {e}")))?;

        let mut cmd = Command::new("gzip");
        cmd.arg("-c").stdin(src).stdout(dest);
        cmd.run()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, dest: P) -> crate::Result<()> {
        let src = &self.path;
        let src = File::open(src)
            .map_err(|e| Error::IO(format!("failed reading archive: {src}: {e}")))?;

        let dest = dest.as_ref();
        let dest = File::create(dest)
            .map_err(|e| Error::IO(format!("failed creating file: {dest}: {e}")))?;

        let mut cmd = Command::new("gzip");
        cmd.arg("-d").arg("-c").stdin(src).stdout(dest);
        cmd.run()
    }
}

#[derive(Debug)]
pub(crate) struct Bz2 {
    path: Utf8PathBuf,
}

impl ArchiveFormat for Bz2 {
    const EXTS: &'static [&'static str] = &["bz2", "bz"];

    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(src: P, dest: Q) -> crate::Result<()> {
        let src = src.as_ref();
        let src = File::open(src)
            .map_err(|e| Error::IO(format!("failed reading file: {src}: {e}")))?;

        let dest = dest.as_ref();
        let dest = File::create(dest)
            .map_err(|e| Error::IO(format!("failed creating file: {dest}: {e}")))?;

        let mut cmd = Command::new("bzip2");
        cmd.arg("-c").stdin(src).stdout(dest);
        cmd.run()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, dest: P) -> crate::Result<()> {
        let src = &self.path;
        let src = File::open(src)
            .map_err(|e| Error::IO(format!("failed reading archive: {src}: {e}")))?;

        let dest = dest.as_ref();
        let dest = File::create(dest)
            .map_err(|e| Error::IO(format!("failed creating file: {dest}: {e}")))?;

        let mut cmd = Command::new("bzip2");
        cmd.arg("-d").arg("-c").stdin(src).stdout(dest);
        cmd.run()
    }
}

#[derive(Debug)]
pub(crate) struct Xz {
    path: Utf8PathBuf,
}

impl ArchiveFormat for Xz {
    const EXTS: &'static [&'static str] = &["xz"];

    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(src: P, dest: Q) -> crate::Result<()> {
        let src = src.as_ref();
        let src = File::open(src)
            .map_err(|e| Error::IO(format!("failed reading file: {src}: {e}")))?;

        let dest = dest.as_ref();
        let dest = File::create(dest)
            .map_err(|e| Error::IO(format!("failed creating file: {dest}: {e}")))?;

        let mut cmd = Command::new("xz");
        cmd.arg("-c").stdin(src).stdout(dest);
        cmd.run()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, dest: P) -> crate::Result<()> {
        let src = &self.path;
        let src = File::open(src)
            .map_err(|e| Error::IO(format!("failed reading archive: {src}: {e}")))?;

        let dest = dest.as_ref();
        let dest = File::create(dest)
            .map_err(|e| Error::IO(format!("failed creating file: {dest}: {e}")))?;

        let mut cmd = Command::new("xz");
        cmd.arg("-d").arg("-c").stdin(src).stdout(dest);
        cmd.run()
    }
}

#[derive(Debug)]
pub(crate) struct _7z {
    path: Utf8PathBuf,
}

impl ArchiveFormat for _7z {
    const EXTS: &'static [&'static str] = &["7z"];

    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(_src: P, _dest: Q) -> crate::Result<()> {
        unimplemented!()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> crate::Result<()> {
        let mut cmd = Command::new("7z");
        cmd.args(["x", "-y", self.path.as_str()]);
        cmd.run()
    }
}

#[derive(Debug)]
pub(crate) struct Rar {
    path: Utf8PathBuf,
}

impl ArchiveFormat for Rar {
    const EXTS: &'static [&'static str] = &["rar"];

    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(_src: P, _dest: Q) -> crate::Result<()> {
        unimplemented!()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> crate::Result<()> {
        let mut cmd = Command::new("unrar");
        cmd.args(["x", "-idq", "-o+", self.path.as_str()]);
        cmd.run()
    }
}

#[derive(Debug)]
pub(crate) struct Lha {
    path: Utf8PathBuf,
}

impl ArchiveFormat for Lha {
    const EXTS: &'static [&'static str] = &["lha", "lzh"];

    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(_src: P, _dest: Q) -> crate::Result<()> {
        unimplemented!()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> crate::Result<()> {
        let mut cmd = Command::new("lha");
        cmd.args(["xfq", self.path.as_str()]);
        cmd.run()
    }
}

#[derive(Debug)]
pub(crate) struct Ar {
    path: Utf8PathBuf,
}

impl ArchiveFormat for Ar {
    const EXTS: &'static [&'static str] = &["deb", "a"];

    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(_src: P, _dest: Q) -> crate::Result<()> {
        unimplemented!()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, _dest: P) -> crate::Result<()> {
        let mut cmd = Command::new("ar");
        cmd.args(["x", self.path.as_str()]);
        cmd.run()
    }
}

#[derive(Debug)]
pub(crate) struct Lzma {
    path: Utf8PathBuf,
}

impl ArchiveFormat for Lzma {
    const EXTS: &'static [&'static str] = &["lzma"];

    fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(src: P, dest: Q) -> crate::Result<()> {
        let src = src.as_ref();
        let src = File::open(src)
            .map_err(|e| Error::IO(format!("failed reading file: {src}: {e}")))?;

        let dest = dest.as_ref();
        let dest = File::create(dest)
            .map_err(|e| Error::IO(format!("failed creating file: {dest}: {e}")))?;

        let mut cmd = Command::new("lzma");
        cmd.arg("-c").stdin(src).stdout(dest);
        cmd.run()
    }

    fn unpack<P: AsRef<Utf8Path>>(&self, dest: P) -> crate::Result<()> {
        let dest = dest.as_ref();
        let dest = File::create(dest)
            .map_err(|e| Error::IO(format!("failed creating file: {dest}: {e}")))?;

        let mut cmd = Command::new("lzma");
        cmd.arg("-dc").arg(&self.path).stdout(dest);
        cmd.run()
    }
}

macro_rules! make_archive {
    ($($x:ident),+) => {
        #[derive(Debug)]
        pub(crate) enum Archive {
            $($x($x),)+
        }

        impl ArchiveFormat for Archive {
            const EXTS: &'static [&'static str] = &[];

            fn pack<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(src: P, dest: Q) -> crate::Result<()> {
                let archive = Archive::from_path(dest.as_ref())?;
                match archive {
                    $(Archive::$x(_) => $x::pack(src, dest),)+
                }
            }

            fn unpack<P: AsRef<Utf8Path>>(&self, dest: P) -> crate::Result<()> {
                match self {
                    $(Archive::$x(a) => a.unpack(dest),)+
                }
            }
        }

        impl Archive {
            pub(crate) fn from_path<P: AsRef<Utf8Path>>(path: P) -> crate::Result<Archive> {
                let path = path.as_ref();
                let path = Utf8PathBuf::from(path);
                let filename = path.file_name().ok_or_else(||
                    Error::InvalidValue(format!("invalid archive: {path}")))?;
                let filename = filename.to_lowercase();

                let mut possible_exts = vec![];
                $(
                    possible_exts.extend($x::EXTS.iter().map(|&s| (s, $x::EXTS[0])));
                )+
                possible_exts.sort_by_cached_key(|(s, _)| s.len());
                possible_exts.reverse();

                let mut marker_ext = "";
                for (ext, marker) in possible_exts {
                    if filename.ends_with(ext) {
                        marker_ext = marker;
                        break;
                    }
                }

                match marker_ext {
                    $(ext if ext == $x::EXTS[0] => Ok(Archive::$x($x { path })),)+
                    _ => Err(Error::InvalidValue(format!("unknown archive format: {path}"))),
                }
            }
        }
    };
}
make_archive!(Tar, TarGz, TarBz2, TarLzma, TarXz, Zip, Gz, Bz2, Xz, _7z, Rar, Lha, Ar, Lzma);
