//! Memory outside the heap, as `Unsafe` gives it out by address: `allocateMemory`,
//! `reallocateMemory` and `freeMemory`, and the reads, writes, fills and copies at an address
//! (with a null base). Direct buffers keep their contents there, and a file channel copies
//! through a temporary one.
//!
//! The translation keeps each allocated block as bytes of its own, at an address of its own that
//! no other block's bytes reach: the blocks are far apart, so that a read past a block's end does
//! not land in the next one. An access that is not inside one block (a freed one, or none) ends
//! the program, where the JVM would read or write other memory, or crash.

use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::natives::RawBytes;

/// The blocks by their address, and the address of the next one.
struct Blocks {
    blocks: BTreeMap<i64, Vec<u8>>,
    next: i64,
}

static MEMORY: Mutex<Blocks> = Mutex::new(Blocks { blocks: BTreeMap::new(), next: FIRST });

/// The address of the first block: above the addresses of small numbers, as the JVM's are.
const FIRST: i64 = 1 << 32;

/// The space between two blocks, beyond the first's last byte.
const GAP: i64 = 1 << 16;

fn memory() -> std::sync::MutexGuard<'static, Blocks> {
    MEMORY.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn outside(what: &'static str) -> ! {
    crate::natives::unsupported_memory(what)
}

impl Blocks {
    fn allocate(&mut self, bytes: i64) -> i64 {
        let Ok(size) = usize::try_from(bytes) else { outside("an allocation of a negative size") };
        let address = self.next;
        // Aligned to 16 bytes, as `malloc` aligns.
        self.next = (address + bytes + GAP + 15) & !15;
        self.blocks.insert(address, vec![0; size]);
        address
    }

    /// The bytes `address..address + len`, inside one block.
    fn range(&mut self, address: i64, len: usize) -> &mut [u8] {
        let found = self.blocks.range_mut(..=address).next_back();
        if let Some((&start, block)) = found
            && let Some(to) = ((address - start) as usize).checked_add(len)
            && to <= block.len()
        {
            return &mut block[(address - start) as usize..to];
        }
        crate::exception::die(format!(
            "an Unsafe access of {len} bytes at address {address:#x}, outside the memory the program \
             allocated (or after it freed it)"
        ))
    }
}

unsafe extern "C" {
    fn sysconf(name: i32) -> std::ffi::c_long;
}

/// The system's page size (`sysconf(_SC_PAGESIZE)`), which the JVM gives `Unsafe.pageSize()`.
pub fn page_size() -> i32 {
    #[cfg(target_os = "linux")]
    const PAGE_SIZE: i32 = 30;
    #[cfg(not(target_os = "linux"))]
    const PAGE_SIZE: i32 = 29;
    // SAFETY: `sysconf` reads a value of the system and has no other effect.
    let size = unsafe { sysconf(PAGE_SIZE) };
    i32::try_from(size).ok().filter(|&s| s > 0).unwrap_or(4096)
}

/// `Unsafe.allocateMemory0(bytes)`: a new block of `bytes` zero bytes (the JVM's are not
/// cleared, and what reads them writes them first).
pub fn allocate_native(bytes: i64) -> i64 {
    memory().allocate(bytes)
}

/// `Unsafe.reallocateMemory0(address, bytes)`: a new block with the old one's bytes, as many as
/// fit, after which the old one is freed.
pub fn reallocate_native(address: i64, bytes: i64) -> i64 {
    let mut memory = memory();
    let Some(old) = memory.blocks.remove(&address) else {
        outside("a reallocation of memory that was not allocated")
    };
    let new = memory.allocate(bytes);
    let block = memory.blocks.get_mut(&new).expect("the new block");
    let kept = old.len().min(block.len());
    block[..kept].copy_from_slice(&old[..kept]);
    new
}

/// `Unsafe.freeMemory0(address)`.
pub fn free_native(address: i64) {
    if memory().blocks.remove(&address).is_none() {
        outside("a release of memory that was not allocated");
    }
}

/// The `len` bytes at `address`.
pub fn read_native(address: i64, len: usize) -> Vec<u8> {
    memory().range(address, len).to_vec()
}

/// Writes `bytes` at `address`.
pub fn write_native(address: i64, bytes: &[u8]) {
    memory().range(address, bytes.len()).copy_from_slice(bytes);
}

/// `Unsafe.getInt(null, address)` and the other plain reads of a primitive at an address.
pub fn get_native<T: RawBytes>(address: i64) -> T {
    T::get(memory().range(address, T::SIZE))
}

/// `Unsafe.putInt(null, address, x)` and the other plain writes of a primitive at an address.
pub fn put_native<T: RawBytes>(address: i64, value: T) {
    value.put(memory().range(address, T::SIZE));
}

/// `Unsafe.setMemory0(null, address, bytes, value)`: `bytes` bytes of `value` from `address`.
pub fn set_native(address: i64, bytes: i64, value: i8) {
    let Ok(len) = usize::try_from(bytes) else { outside("a fill of a negative size") };
    memory().range(address, len).fill(value as u8);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_size_is_a_power_of_two() {
        assert!(page_size() >= 4096 && page_size().count_ones() == 1);
    }

    #[test]
    fn keeps_blocks_apart() {
        let a = allocate_native(8);
        let b = allocate_native(4);
        assert!(b >= a + 8 + GAP);
        assert_eq!(a % 16, 0);
        put_native::<i64>(a, 0x0102_0304_0506_0708);
        assert_eq!(get_native::<i8>(a), 8);
        assert_eq!(get_native::<i32>(a + 4), 0x0102_0304);
        set_native(b, 4, 7);
        assert_eq!(read_native(b, 4), vec![7; 4]);
        let c = reallocate_native(a, 16);
        assert_eq!(get_native::<i64>(c), 0x0102_0304_0506_0708);
        assert_eq!(get_native::<i64>(c + 8), 0);
        write_native(c + 8, &[1, 2]);
        assert_eq!(get_native::<i16>(c + 8), 0x0201);
        free_native(c);
        free_native(b);
    }
}
