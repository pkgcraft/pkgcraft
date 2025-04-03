use std::fs::File;
use std::os::fd::{AsFd, AsRawFd};

use ipc_channel::ipc::{self, IpcError, IpcReceiver, IpcSender};
use nix::errno::Errno;
use nix::unistd::{dup2, fork, ForkResult};
use serde::{Deserialize, Serialize};

use crate::shm::create_shm;
use crate::{bash, shell, Error};

/// Redirect stdout and stderr to a given raw file descriptor.
pub fn redirect_output<T: AsFd>(f: T) -> crate::Result<()> {
    let fd = f.as_fd().as_raw_fd();
    dup2(fd, 1)?;
    dup2(fd, 2)?;
    Ok(())
}

/// Suppress stdout and stderr.
pub fn suppress_output() -> crate::Result<()> {
    let f = File::options().write(true).open("/dev/null")?;
    redirect_output(&f)?;
    Ok(())
}

/// Semaphore wrapping libc semaphore calls on top of shared memory.
pub struct SharedSemaphore {
    sem: *mut libc::sem_t,
    size: u32,
}

impl SharedSemaphore {
    pub fn new(size: usize) -> crate::Result<Self> {
        let ptr = create_shm("scallop-pool-sem", std::mem::size_of::<libc::sem_t>())?;
        let sem = ptr as *mut libc::sem_t;

        // sem_init() uses u32 values
        let size: u32 = size
            .try_into()
            .map_err(|_| Error::Base(format!("pool too large: {size}")))?;

        if unsafe { libc::sem_init(sem, 1, size) } == 0 {
            Ok(Self { sem, size })
        } else {
            let err = Errno::last_raw();
            Err(Error::Base(format!("sem_init() failed: {err}")))
        }
    }

    pub fn acquire(&mut self) -> crate::Result<()> {
        if unsafe { libc::sem_wait(self.sem) } == 0 {
            Ok(())
        } else {
            let err = Errno::last_raw();
            Err(Error::Base(format!("sem_wait() failed: {err}")))
        }
    }

    pub fn release(&mut self) -> crate::Result<()> {
        if unsafe { libc::sem_post(self.sem) } == 0 {
            Ok(())
        } else {
            let err = Errno::last_raw();
            Err(Error::Base(format!("sem_post() failed: {err}")))
        }
    }

    pub fn wait(&mut self) -> crate::Result<()> {
        for _ in 0..self.size {
            self.acquire()?;
        }
        Ok(())
    }
}

impl Drop for SharedSemaphore {
    fn drop(&mut self) {
        unsafe { libc::sem_destroy(self.sem) };
    }
}

pub struct PoolIter<T: Serialize + for<'a> Deserialize<'a>> {
    rx: IpcReceiver<T>,
}

impl<T: Serialize + for<'a> Deserialize<'a>> PoolIter<T> {
    pub fn new<O, I, F>(size: usize, iter: I, func: F, suppress: bool) -> crate::Result<Self>
    where
        I: Iterator<Item = O>,
        F: FnOnce(O) -> T,
    {
        let mut sem = SharedSemaphore::new(size)?;
        let (tx, rx): (IpcSender<T>, IpcReceiver<T>) = ipc::channel()
            .map_err(|e| Error::Base(format!("failed creating IPC channel: {e}")))?;

        match unsafe { fork() } {
            Ok(ForkResult::Parent { .. }) => Ok(()),
            Ok(ForkResult::Child) => {
                shell::fork_init();
                // enable internal bash SIGCHLD handler
                unsafe { bash::set_sigchld_handler() };

                if suppress {
                    // suppress stdout and stderr in forked processes
                    suppress_output()?;
                }

                for obj in iter {
                    // wait on bounded semaphore for pool space
                    sem.acquire()?;

                    match unsafe { fork() } {
                        Ok(ForkResult::Parent { .. }) => (),
                        Ok(ForkResult::Child) => {
                            shell::fork_init();
                            // TODO: use catch_unwind() with UnwindSafe function and serialize tracebacks
                            let r = func(obj);
                            tx.send(r).map_err(|e| {
                                Error::Base(format!("process pool sending failed: {e}"))
                            })?;
                            sem.release()?;
                            unsafe { libc::_exit(0) };
                        }
                        Err(e) => panic!("process pool fork failed: {e}"),
                    }
                }
                unsafe { libc::_exit(0) };
            }
            Err(e) => Err(Error::Base(format!("starting process pool failed: {e}"))),
        }?;

        Ok(Self { rx })
    }
}

impl<T: Serialize + for<'a> Deserialize<'a>> Iterator for PoolIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match self.rx.recv() {
            Ok(r) => Some(r),
            Err(IpcError::Disconnected) => None,
            Err(e) => panic!("process pool receiver failed: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::source;
    use crate::variables::optional;

    use super::*;

    #[test]
    fn env_leaking() {
        assert!(optional("VAR").is_none());

        let vals: Vec<_> = (0..16).collect();
        let func = |i: u64| {
            source::string(format!("VAR={i}")).unwrap();
            assert_eq!(optional("VAR").unwrap(), i.to_string());
            i
        };

        PoolIter::new(2, vals.into_iter(), func, false)
            .unwrap()
            .for_each(drop);

        assert!(optional("VAR").is_none());
    }
}
