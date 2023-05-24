use std::panic::{catch_unwind, UnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};

use nix::errno::errno;
use nix::unistd::{fork, ForkResult};

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
    fn new(size: u32) -> crate::Result<Self> {
        let pid = std::process::id();
        let id = get_id();
        let shm_name = format!("scallop-pool-sem-{pid}-{id}");
        let ptr = create_shm(&shm_name, std::mem::size_of::<libc::sem_t>())?;
        let sem = ptr as *mut libc::sem_t;

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
}

impl Pool {
    pub fn new(size: usize) -> crate::Result<Self> {
        // enable internal bash SIGCHLD handler
        unsafe { bash::set_sigchld_handler() };

        let sem_size = size.try_into()
            .map_err(|_| Error::Base(format!("pool too large: {size}")))?;
        let sem = SharedSemaphore::new(sem_size)?;

        Ok(Self { sem } )
    }

    /// Spawn a new, forked process if space is available in the pool, otherwise wait for space.
    pub fn spawn<F>(&mut self, func: F) -> crate::Result<()>
    where
        F: FnOnce() + UnwindSafe,
    {
        // wait on bounded semaphore for pool space
        self.sem.acquire()?;

        match unsafe { fork() } {
            Ok(ForkResult::Parent { .. }) => {
                Ok(())
            }
            Ok(ForkResult::Child) => {
                // TODO: serialize errors/tracebacks and send them back to parent
                let _ = catch_unwind(func);
                self.sem.release().ok();
                unsafe { libc::_exit(0) };
            }
            Err(e) => Err(Error::Base(format!("fork failed: {e}"))),
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
            pool.spawn(|| {
                source::string(format!("VAR={i}")).unwrap();
                assert_eq!(optional("VAR").unwrap(), i.to_string());
            })
            .unwrap();
        }

        assert!(optional("VAR").is_none());
    }

    #[test]
    fn panic_handling() {
        let mut pool = Pool::new(2).unwrap();
        for _ in 0..8 {
            pool.spawn(|| {
                source::string("exit 0").unwrap();
            })
            .unwrap();
        }
    }
}
