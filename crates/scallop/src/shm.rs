use std::ffi::c_void;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};

use nix::fcntl::OFlag;
use nix::sys::mman::{MapFlags, ProtFlags, mmap, shm_open, shm_unlink};
use nix::sys::stat::Mode;
use nix::unistd::ftruncate;

use crate::Error;

/// Get a unique ID for shared memory names.
fn get_id() -> usize {
    static COUNTER: AtomicUsize = AtomicUsize::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Create a shared memory object with a given size.
pub(crate) fn create_shm(prefix: &str, size: usize) -> crate::Result<*mut c_void> {
    let pid = std::process::id();
    let id = get_id();
    let name = format!("/{prefix}-{pid}-{id}");

    // create shared memory object
    let shm_fd = shm_open(
        name.as_str(),
        OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_RDWR,
        Mode::S_IRUSR | Mode::S_IWUSR,
    )
    .map_err(|e| Error::Base(format!("shm_open(): {e}")))?;

    // enlarge file to the given size
    ftruncate(&shm_fd, size as i64).map_err(|e| Error::Base(format!("ftruncate(): {e}")))?;

    // map file into memory
    let length = NonZeroUsize::new(size)
        .ok_or_else(|| Error::Base("size must be nonzero".to_string()))?;
    let shm_ptr = unsafe {
        mmap(
            None,
            length,
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
            MapFlags::MAP_SHARED,
            &shm_fd,
            0,
        )
        .map_err(|e| Error::Base(format!("mmap(): {e}")))?
        .as_mut()
    };

    shm_unlink(name.as_str()).map_err(|e| Error::Base(format!("shm_unlink(): {e}")))?;

    Ok(shm_ptr)
}
