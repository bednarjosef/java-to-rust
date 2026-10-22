//! The file channel natives (`ast::Native::TransferUnsupported`, `MapFile`, `UnmapFile` and
//! `AllocationGranularity`): what `FileChannelImpl` asks of the operating system to transfer bytes
//! between channels, and to map a file. A transfer is answered `IOStatus.UNSUPPORTED_CASE`, so the
//! JDK copies through its buffers. A mapping is a block outside the heap (`native_memory`) with the
//! file's bytes, copied when the mapping is made.

use crate::fd::{Failure, last_error, with_file};
use crate::native_memory::{allocate_native, free_native, page_size, write_native};
use crate::natives::unsupported_memory;
use std::io::{self, ErrorKind};
use std::os::unix::fs::FileExt;

/// `IOStatus.UNSUPPORTED_CASE`: the transfer is not done here, and the JDK copies through buffers.
const UNSUPPORTED_CASE: i64 = -6;

/// `FileChannelImpl`'s `MAP_RW`: the mode of a mapping whose changes reach the file.
const MAP_RW: i32 = 1;

/// `FileDispatcherImpl.transferTo0` and `transferFrom0`. The runtime does not copy inside the
/// kernel: it answers `UNSUPPORTED_CASE`, and `FileChannelImpl` copies through its buffers at the
/// positions and for the counts that Java gives, so the bytes and the positions are the same.
pub fn transfer_unsupported() -> i64 {
    UNSUPPORTED_CASE
}

/// `UnixFileDispatcherImpl.map0`: a block outside the heap that holds the bytes of the file from
/// `position`, for `length` bytes. Past the end of the file the bytes are zeros, as a file that is
/// extended reads. The block is a copy, so a later change to the file is not seen through it, and
/// a change through the buffer does not reach the file. A read-write mapping, whose changes must
/// reach the file, and a synchronous one end the program.
pub fn map_file(fd: i32, prot: i32, position: i64, length: i64, is_sync: bool) -> Result<i64, Failure> {
    if prot == MAP_RW || is_sync {
        unsupported_memory("a read-write or synchronous mapping of a file")
    }
    let mut bytes = vec![0u8; length as usize];
    with_file(fd, |file| {
        let mut done = 0;
        while done < bytes.len() {
            match file.read_at(&mut bytes[done..], position as u64 + done as u64) {
                Ok(0) => break,
                Ok(n) => done += n,
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok::<(), io::Error>(())
    })
    .map_err(last_error)?;
    let address = allocate_native(length);
    write_native(address, &bytes);
    Ok(address)
}

/// `UnixFileDispatcherImpl.unmap0`: frees the block that `map_file` made. Returns 0, as the JDK's
/// does when the unmapping succeeds.
pub fn unmap_file(address: i64, _length: i64) -> i32 {
    free_native(address);
    0
}

/// `UnixFileDispatcherImpl.allocationGranularity0`: the unit that a mapping's position is aligned
/// to, which is the page size.
pub fn allocation_granularity() -> i64 {
    page_size() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fd::{fd_close, fd_open_read};
    use crate::native_memory::{read_native, write_native};

    /// A file in the temporary directory, with `contents`, under a name that is unique to the test.
    fn temp_file(test: &str, contents: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("java-runtime-file-channels-{test}"));
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn transfer_is_left_to_the_jdk() {
        assert_eq!(transfer_unsupported(), -6);
    }

    #[test]
    fn read_only_map_holds_the_file_from_a_position() {
        let path = temp_file("read-only", b"0123456789abcdef");
        let fd = fd_open_read(path.to_str().unwrap()).unwrap();
        let address = map_file(fd, 0, 4, 6, false).unwrap();
        assert_eq!(read_native(address, 6), b"456789".to_vec());
        let past_end = map_file(fd, 0, 14, 4, false).unwrap();
        assert_eq!(read_native(past_end, 4), b"ef\0\0".to_vec());
        assert_eq!(unmap_file(address, 6), 0);
        assert_eq!(unmap_file(past_end, 4), 0);
        fd_close(fd);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn private_map_is_a_copy() {
        let path = temp_file("private", b"abcdef");
        let fd = fd_open_read(path.to_str().unwrap()).unwrap();
        let address = map_file(fd, 2, 0, 6, false).unwrap();
        write_native(address, b"XY");
        assert_eq!(read_native(address, 6), b"XYcdef".to_vec());
        assert_eq!(std::fs::read(&path).unwrap(), b"abcdef".to_vec());
        assert_eq!(unmap_file(address, 6), 0);
        fd_close(fd);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn granularity_is_the_page_size() {
        assert_eq!(allocation_granularity(), page_size() as i64);
        assert!(allocation_granularity() >= 4096);
    }
}
