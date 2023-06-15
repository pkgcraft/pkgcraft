use std::fs::File;
use std::os::fd::{AsRawFd, RawFd};
use std::sync::atomic::{AtomicUsize, Ordering};

use ipc_channel::ipc::{self, IpcError, IpcReceiver, IpcSender};
use nix::errno::errno;
use nix::unistd::{close, dup2, fork, ForkResult};
use serde::{Deserialize, Serialize};

use crate::shm::create_shm;
use crate::{bash, Error};

/// Get a unique ID for shared memory names.
fn get_id() -> usize {
    static COUNTER: AtomicUsize = AtomicUsize::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Redirect stdout and stderr to a given raw file descriptor.
pub fn redirect_output(fd: RawFd) -> crate::Result<()> {
    dup2(fd, 1)?;
    dup2(fd, 2)?;
    close(fd)?;
    Ok(())
}

/// Suppress stdout and stderr.
pub fn suppress_output() -> crate::Result<()> {
    let f = File::options().write(true).open("/dev/null")?;
    redirect_output(f.as_raw_fd())?;
    Ok(())
}

/// Semaphore wrapping libc semaphore calls on top of shared memory.
struct SharedSemaphore {
    sem: *mut libc::sem_t,
    size: u32,
}

impl SharedSemaphore {
    fn new(size: usize) -> crate::Result<Self> {
        let pid = std::process::id();
        let id = get_id();
        let name = format!("/scallop-pool-sem-{pid}-{id}");
        let ptr = create_shm(&name, std::mem::size_of::<libc::sem_t>())?;
        let sem = ptr as *mut libc::sem_t;

        // sem_init() uses u32 values
        let size: u32 = size
            .try_into()
            .map_err(|_| Error::Base(format!("pool too large: {size}")))?;

        if unsafe { libc::sem_init(sem, 1, size) } == 0 {
            Ok(Self { sem, size })
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

    fn wait(&mut self) -> crate::Result<()> {
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
        // enable internal bash SIGCHLD handler
        unsafe { bash::set_sigchld_handler() };

        let mut sem = SharedSemaphore::new(size)?;
        let (tx, rx): (IpcSender<T>, IpcReceiver<T>) =
            ipc::channel().map_err(|e| Error::Base(format!("failed creating IPC channel: {e}")))?;

        match unsafe { fork() } {
            Ok(ForkResult::Parent { .. }) => Ok(()),
            Ok(ForkResult::Child) => {
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

#[derive(Debug, Serialize, Deserialize)]
enum Msg<T> {
    Val(T),
    Stop,
}

pub struct PoolSendIter<I, O>
where
    I: Serialize + for<'a> Deserialize<'a>,
    O: Serialize + for<'a> Deserialize<'a>,
{
    input_tx: IpcSender<Msg<I>>,
    output_rx: IpcReceiver<Msg<O>>,
}

impl<I, O> PoolSendIter<I, O>
where
    I: Serialize + for<'a> Deserialize<'a>,
    O: Serialize + for<'a> Deserialize<'a>,
{
    pub fn new<F>(size: usize, func: F, suppress: bool) -> crate::Result<Self>
    where
        F: FnOnce(I) -> O,
    {
        // enable internal bash SIGCHLD handler
        unsafe { bash::set_sigchld_handler() };

        let mut sem = SharedSemaphore::new(size)?;
        let (input_tx, input_rx): (IpcSender<Msg<I>>, IpcReceiver<Msg<I>>) = ipc::channel()
            .map_err(|e| Error::Base(format!("failed creating input channel: {e}")))?;
        let (output_tx, output_rx): (IpcSender<Msg<O>>, IpcReceiver<Msg<O>>) = ipc::channel()
            .map_err(|e| Error::Base(format!("failed creating output channel: {e}")))?;

        match unsafe { fork() } {
            Ok(ForkResult::Parent { .. }) => Ok(()),
            Ok(ForkResult::Child) => {
                if suppress {
                    // suppress stdout and stderr in forked processes
                    suppress_output()?;
                }

                while let Ok(Msg::Val(obj)) = input_rx.recv() {
                    // wait on bounded semaphore for pool space
                    sem.acquire()?;
                    match unsafe { fork() } {
                        Ok(ForkResult::Parent { .. }) => (),
                        Ok(ForkResult::Child) => {
                            // TODO: use catch_unwind() with UnwindSafe function and serialize tracebacks
                            let r = func(obj);
                            output_tx.send(Msg::Val(r)).map_err(|e| {
                                Error::Base(format!("process pool failed send: {e}"))
                            })?;
                            sem.release()?;
                            unsafe { libc::_exit(0) };
                        }
                        Err(e) => panic!("process pool fork failed: {e}"),
                    }
                }

                // wait for forked processes to complete
                sem.wait()?;

                // signal iterator end
                output_tx
                    .send(Msg::Stop)
                    .map_err(|e| Error::Base(format!("process pool failed stop: {e}")))?;

                unsafe { libc::_exit(0) }
            }
            Err(e) => Err(Error::Base(format!("process pool failed start: {e}"))),
        }?;

        Ok(Self { input_tx, output_rx })
    }

    /// Create a new forked process pool, sending the given data to it for processing.
    pub fn iter<V: Iterator<Item = I>>(&self, vals: V) -> crate::Result<PoolReceiveIter<O>> {
        // queue data in a separate process
        match unsafe { fork() } {
            Ok(ForkResult::Parent { .. }) => Ok(()),
            Ok(ForkResult::Child) => {
                // send values to process pool
                for val in vals {
                    self.input_tx
                        .send(Msg::Val(val))
                        .map_err(|e| Error::Base(format!("failed queuing value: {e}")))?;
                }

                // signal process pool end
                self.input_tx
                    .send(Msg::Stop)
                    .map_err(|e| Error::Base(format!("failed stopping workers: {e}")))?;

                unsafe { libc::_exit(0) };
            }
            Err(e) => Err(Error::Base(format!("failed starting queuing process: {e}"))),
        }?;

        Ok(PoolReceiveIter { rx: &self.output_rx })
    }
}

impl<I, O> Drop for PoolSendIter<I, O>
where
    I: Serialize + for<'a> Deserialize<'a>,
    O: Serialize + for<'a> Deserialize<'a>,
{
    fn drop(&mut self) {
        self.input_tx.send(Msg::Stop).ok();
    }
}

pub struct PoolReceiveIter<'p, T>
where
    T: Serialize + for<'a> Deserialize<'a>,
{
    rx: &'p IpcReceiver<Msg<T>>,
}

impl<T> Iterator for PoolReceiveIter<'_, T>
where
    T: Serialize + for<'a> Deserialize<'a>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match self.rx.recv() {
            Ok(Msg::Val(r)) => Some(r),
            Ok(Msg::Stop) => None,
            Err(e) => panic!("output receiver failed: {e}"),
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
