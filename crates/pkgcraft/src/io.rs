use std::io::{self, Cursor, Error, ErrorKind, Read, Write};
use std::sync::{Mutex, OnceLock};

use crate::cli::is_terminal;

pub(crate) fn stdin() -> Stdin {
    static INSTANCE: OnceLock<Mutex<StdinInternal>> = OnceLock::new();
    Stdin {
        inner: INSTANCE.get_or_init(|| {
            Mutex::new(if cfg!(test) {
                StdinInternal::Fake(Cursor::default())
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
        if let Ok(StdinInternal::Fake(f)) = self.inner.lock().as_deref_mut() {
            let result = f.write(data.as_bytes());
            f.set_position(0);
            result
        } else {
            unreachable!("stdin injection only valid during testing");
        }
    }
}

impl Read for Stdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.inner.lock().as_deref_mut() {
            Ok(StdinInternal::Real(f)) => {
                if is_terminal!(f) {
                    Err(Error::new(ErrorKind::Other, "stdin is a terminal"))
                } else {
                    f.read(buf)
                }
            }
            Ok(StdinInternal::Fake(f)) => f.read(buf),
            Err(e) => Err(Error::new(ErrorKind::Other, format!("failed getting stdin: {e}"))),
        }
    }
}

impl Write for Stdin {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.inner.lock().as_deref_mut() {
            Ok(StdinInternal::Fake(f)) => f.write(buf),
            _ => Err(Error::new(ErrorKind::Other, "failed getting stdin")),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.inner.lock().as_deref_mut() {
            Ok(StdinInternal::Fake(f)) => f.flush(),
            _ => Err(Error::new(ErrorKind::Other, "failed getting stdin")),
        }
    }
}

pub(crate) fn stdout() -> Stdout {
    static INSTANCE: OnceLock<Mutex<StdoutInternal>> = OnceLock::new();
    Stdout {
        inner: INSTANCE.get_or_init(|| {
            Mutex::new(if cfg!(not(test)) || scallop::shell::in_subshell() {
                StdoutInternal::Real(io::stdout())
            } else {
                StdoutInternal::Fake(Cursor::default())
            })
        }),
    }
}

enum StdoutInternal {
    Real(io::Stdout),
    Fake(Cursor<Vec<u8>>),
}

pub(crate) struct Stdout {
    inner: &'static Mutex<StdoutInternal>,
}

impl Stdout {
    /// Assert stdout data for testing.
    #[cfg(test)]
    pub(crate) fn get(&mut self) -> String {
        if let Ok(StdoutInternal::Fake(f)) = self.inner.lock().as_deref_mut() {
            f.set_position(0);
            String::from_utf8(std::mem::take(f.get_mut())).unwrap()
        } else {
            unreachable!("stdout assertion only valid during testing");
        }
    }
}

impl Read for Stdout {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.inner.lock().as_deref_mut() {
            Ok(StdoutInternal::Fake(f)) => f.read(buf),
            _ => Err(Error::new(ErrorKind::Other, "failed getting stdout")),
        }
    }
}

impl Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.inner.lock().as_deref_mut() {
            Ok(StdoutInternal::Fake(f)) => f.write(buf),
            Ok(StdoutInternal::Real(f)) => f.write(buf),
            Err(_) => Err(Error::new(ErrorKind::Other, "failed getting stdout")),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.inner.lock().as_deref_mut() {
            Ok(StdoutInternal::Fake(f)) => f.flush(),
            Ok(StdoutInternal::Real(f)) => f.flush(),
            Err(_) => Err(Error::new(ErrorKind::Other, "failed getting stdout")),
        }
    }
}

pub(crate) fn stderr() -> Stderr {
    static INSTANCE: OnceLock<Mutex<StderrInternal>> = OnceLock::new();
    Stderr {
        inner: INSTANCE.get_or_init(|| {
            Mutex::new(if cfg!(not(test)) || scallop::shell::in_subshell() {
                StderrInternal::Real(io::stderr())
            } else {
                StderrInternal::Fake(Cursor::default())
            })
        }),
    }
}

enum StderrInternal {
    Real(io::Stderr),
    Fake(Cursor<Vec<u8>>),
}

pub(crate) struct Stderr {
    inner: &'static Mutex<StderrInternal>,
}

impl Stderr {
    /// Assert stderr data for testing.
    #[cfg(test)]
    pub(crate) fn get(&mut self) -> String {
        if let Ok(StderrInternal::Fake(f)) = self.inner.lock().as_deref_mut() {
            f.set_position(0);
            String::from_utf8(std::mem::take(f.get_mut())).unwrap()
        } else {
            unreachable!("stderr assertion only valid during testing");
        }
    }
}

impl Read for Stderr {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.inner.lock().as_deref_mut() {
            Ok(StderrInternal::Fake(f)) => f.read(buf),
            _ => Err(Error::new(ErrorKind::Other, "failed getting stderr")),
        }
    }
}

impl Write for Stderr {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.inner.lock().as_deref_mut() {
            Ok(StderrInternal::Fake(f)) => f.write(buf),
            Ok(StderrInternal::Real(f)) => f.write(buf),
            Err(_) => Err(Error::new(ErrorKind::Other, "failed getting stderr")),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.inner.lock().as_deref_mut() {
            Ok(StderrInternal::Fake(f)) => f.flush(),
            Ok(StderrInternal::Real(f)) => f.flush(),
            Err(_) => Err(Error::new(ErrorKind::Other, "failed getting stderr")),
        }
    }
}
