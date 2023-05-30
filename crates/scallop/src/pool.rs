use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use ipc_channel::ipc::{self, IpcError, IpcReceiver, IpcSender};
use nix::errno::errno;
use nix::unistd::{fork, ForkResult};
use serde::{Deserialize, Serialize};

use crate::shm::create_shm;
use crate::{bash, Error};

/// Get a unique ID for shared memory names.
fn get_id() -> usize {
    static COUNTER: AtomicUsize = AtomicUsize::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Semaphore wrapping libc semaphore calls on top of shared memory.
struct SharedSemaphore {
    sem: *mut libc::sem_t,
}

impl SharedSemaphore {
    fn new(size: usize) -> crate::Result<Self> {
        let pid = std::process::id();
        let id = get_id();
        let shm_name = format!("/scallop-pool-sem-{pid}-{id}");
        let ptr = create_shm(&shm_name, std::mem::size_of::<libc::sem_t>())?;
        let sem = ptr as *mut libc::sem_t;

        // sem_init() uses u32 values
        let size: u32 = size
            .try_into()
            .map_err(|_| Error::Base(format!("pool too large: {size}")))?;

        if unsafe { libc::sem_init(sem, 1, size) } == 0 {
            Ok(Self { sem })
        } else {
            let err = errno();
            Err(Error::Base(format!("sem_init() failed: {err}")))
        }
    }

    fn acquire(&mut self) -> crate::Result<()> {
        if unsafe { libc::sem_wait(self.sem) } == 0 {
            Ok(())
        } else {
            let err = errno();
            Err(Error::Base(format!("sem_wait() failed: {err}")))
        }
    }

    fn release(&mut self) -> crate::Result<()> {
        if unsafe { libc::sem_post(self.sem) } == 0 {
            Ok(())
        } else {
            let err = errno();
            Err(Error::Base(format!("sem_post() failed: {err}")))
        }
    }
}

pub struct Pool {
    sem: SharedSemaphore,
    tx: IpcSender<Error>,
    thread: thread::JoinHandle<usize>,
}

impl Pool {
    pub fn new(size: usize) -> crate::Result<Self> {
        // enable internal bash SIGCHLD handler
        unsafe { bash::set_sigchld_handler() };

        let sem = SharedSemaphore::new(size)?;
        let (tx, rx): (IpcSender<Error>, IpcReceiver<Error>) =
            ipc::channel().map_err(|e| Error::Base(format!("failed creating IPC channel: {e}")))?;

        let mut errors = 0;
        let thread = thread::spawn(move || loop {
            match rx.recv() {
                Ok(e) => {
                    errors += 1;
                    eprintln!("{e}")
                }
                Err(IpcError::Disconnected) => return errors,
                Err(e) => panic!("pool receiver failed: {e}"),
            }
        });

        Ok(Self { sem, tx, thread })
    }

    /// Spawn a new, forked process if space is available in the pool, otherwise wait for space.
    pub fn spawn<F>(&mut self, func: F) -> crate::Result<()>
    where
        F: FnOnce() -> crate::Result<()>,
    {
        // wait on bounded semaphore for pool space
        self.sem.acquire()?;

        match unsafe { fork() } {
            Ok(ForkResult::Parent { .. }) => Ok(()),
            Ok(ForkResult::Child) => {
                // TODO: use catch_unwind() with UnwindSafe function and serialize tracebacks
                if let Err(e) = func() {
                    self.tx.send(e).expect("pool sender failed");
                }
                self.sem.release().expect("failed releasing pool token");
                unsafe { libc::_exit(0) };
            }
            Err(e) => Err(Error::Base(format!("fork failed: {e}"))),
        }
    }

    pub fn join(self) -> crate::Result<usize> {
        // drop sender to signal receiving thread to exit
        drop(self.tx);
        self.thread
            .join()
            .map_err(|_| Error::Base("failed closing pool".to_string()))
    }
}

pub struct PoolIter<T: Serialize + for<'a> Deserialize<'a>> {
    rx: IpcReceiver<T>,
}

impl<T: Serialize + for<'a> Deserialize<'a>> PoolIter<T> {
    pub fn new<O, I, F>(size: usize, iter: I, func: F) -> crate::Result<Self>
    where
        I: Iterator<Item = O>,
        F: FnOnce(O) -> T,
    {
        // enable internal bash SIGCHLD handler
        unsafe { bash::set_sigchld_handler() };

        let mut sem = SharedSemaphore::new(size)?;
        let (tx, rx): (IpcSender<T>, IpcReceiver<T>) =
            ipc::channel().map_err(|e| Error::Base(format!("failed creating IPC channel: {e}")))?;

        match unsafe { fork() } {
            Ok(ForkResult::Parent { .. }) => Ok(()),
            Ok(ForkResult::Child) => {
                for obj in iter {
                    // wait on bounded semaphore for pool space
                    sem.acquire().expect("failed acquiring pool token");

                    match unsafe { fork() } {
                        Ok(ForkResult::Parent { .. }) => (),
                        Ok(ForkResult::Child) => {
                            // TODO: use catch_unwind() with UnwindSafe function and serialize tracebacks
                            let r = func(obj);
                            tx.send(r).expect("process pool sender failed");
                            sem.release().expect("failed releasing pool token");
                            unsafe { libc::_exit(0) };
                        }
                        Err(_) => panic!("process pool fork failed"),
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

        let mut pool = Pool::new(2).unwrap();
        for i in 0..8 {
            pool.spawn(|| -> crate::Result<()> {
                source::string(format!("VAR={i}")).unwrap();
                assert_eq!(optional("VAR").unwrap(), i.to_string());
                Ok(())
            })
            .unwrap();
        }
        pool.join().unwrap();

        assert!(optional("VAR").is_none());
    }

    // TODO: add panic handling tests once catch_unwind() is used
    /*#[test]
    fn panic_handling() {
        let mut pool = Pool::new(2).unwrap();
        for _ in 0..8 {
            pool.spawn(|| -> crate::Result<()> {
                source::string("exit 0").unwrap();
                Ok(())
            })
            .unwrap();
        }
        pool.join().unwrap();
    }*/
}
