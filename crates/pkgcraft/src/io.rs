use std::io::{self, Cursor, Error, ErrorKind, Read, Write};
use std::sync::{Mutex, OnceLock};

use crate::cli::is_terminal;

pub(crate) fn stdin() -> Stdin {
    static INSTANCE: OnceLock<Mutex<StdinInternal>> = OnceLock::new();
    Stdin {
        inner: INSTANCE.get_or_init(|| {
            Mutex::new(if cfg!(test) && std::env::var_os("PKGCRAFT_IO_REAL").is_none() {
                StdinInternal::Fake(Cursor::new(vec![]))
            } else {
                StdinInternal::Real(io::stdin())
            })
        }),
    }
}

enum StdinInternal {
    Real(io::Stdin),
    Fake(Cursor<Vec<u8>>),
}

pub(crate) struct Stdin {
    inner: &'static Mutex<StdinInternal>,
}

impl Stdin {
    /// Inject data into fake stdin for testing.
    #[cfg(test)]
    pub(crate) fn inject(&mut self, data: &str) -> io::Result<usize> {
        if let Ok(StdinInternal::Fake(fake)) = self.inner.lock().as_deref_mut() {
            let r = fake.write(data.as_bytes());
            fake.set_position(0);
            r
        } else {
            Err(Error::new(ErrorKind::Other, "stdin injection only valid during testing"))
        }
    }
}

impl Read for Stdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.inner.lock().as_deref_mut() {
            Ok(StdinInternal::Real(stdin)) => {
                if is_terminal!(stdin) {
                    Err(Error::new(ErrorKind::Other, "stdin is a terminal"))
                } else {
                    stdin.read(buf)
                }
            }
            Ok(StdinInternal::Fake(fake)) => fake.read(buf),
            Err(e) => Err(Error::new(ErrorKind::Other, format!("failed getting stdin: {e}"))),
        }
    }
}

impl Write for Stdin {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.inner.lock().as_deref_mut() {
            Ok(StdinInternal::Fake(fake)) => fake.write(buf),
            _ => Err(Error::new(ErrorKind::Other, "failed getting stdin")),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.inner.lock().as_deref_mut() {
            Ok(StdinInternal::Fake(fake)) => fake.flush(),
            _ => Err(Error::new(ErrorKind::Other, "failed getting stdin")),
        }
    }
}
