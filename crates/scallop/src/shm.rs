use std::ffi::c_void;
use std::num::NonZeroUsize;

use nix::fcntl::OFlag;
use nix::sys::mman::{mmap, shm_open, shm_unlink, MapFlags, ProtFlags};
use nix::sys::stat::Mode;
use nix::unistd::{close, ftruncate};

use crate::Error;

/// Create a shared memory object with a given size.
pub(crate) fn create_shm(id: &str, size: usize) -> crate::Result<*mut c_void> {
    // create shared memory object
    let shm_fd =
        shm_open(id, OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_RDWR, Mode::S_IRUSR | Mode::S_IWUSR)
            .map_err(|e| Error::Base(format!("shm_open(): {e}")))?;

    // enlarge file to the given size
    ftruncate(shm_fd, size as i64).map_err(|e| Error::Base(format!("ftruncate(): {e}")))?;

    // map file into memory
    let length =
        NonZeroUsize::new(size).ok_or_else(|| Error::Base("size must be nonzero".to_string()))?;
    let shm_ptr = unsafe {
        mmap(
            None,
            length,
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
            MapFlags::MAP_SHARED,
            shm_fd,
            0,
        )
        .map_err(|e| Error::Base(format!("mmap(): {e}")))?
    };

    close(shm_fd)?;
    shm_unlink(id).map_err(|e| Error::Base(format!("shm_unlink(): {e}")))?;

    Ok(shm_ptr)
}
