//! Times allocation: the instructions and cycles each `java::Arc::new` takes, collections
//! included, for objects that die young and for objects of which some live on, which leaves
//! the heap's blocks with short holes, as a server's tick does.
//!
//!     cargo run --release -p java-runtime --example allocation [millions]
//!
//! Each line is one pattern: the calling thread's instructions and cycles per allocation (the
//! fewest of 5 rounds, which vary less than times on a busy machine), and the collections.

use java_runtime as java;
use std::hint::black_box;

#[global_allocator]
static ALLOCATOR: java::Allocator = java::Allocator;

/// An object of 24 bytes.
struct Small {
    next: Option<java::Arc<Small>>,
    value: u64,
    count: u32,
}

/// An object of 48 bytes.
struct Medium {
    left: Option<java::Arc<Small>>,
    right: Option<java::Arc<Medium>>,
    numbers: [u64; 3],
    flag: bool,
}

/// A counter of the calling thread's cycles (0) or instructions (1) in user code, or -1.
fn counter(config: u64) -> i32 {
    unsafe extern "C" {
        fn syscall(n: i64, ...) -> i64;
    }
    let mut attr = [0u64; 17];
    // type PERF_TYPE_HARDWARE (0), size; config; flags: exclude_kernel, exclude_hv.
    attr[0] = (std::mem::size_of_val(&attr) as u64) << 32;
    attr[1] = config;
    attr[5] = (1 << 5) | (1 << 6);
    unsafe { syscall(298, attr.as_ptr(), 0i32, -1i32, -1i32, 0u64) as i32 }
}

fn read_counter(fd: i32) -> u64 {
    use std::io::Read;
    use std::os::fd::FromRawFd;
    let mut value = [0u8; 8];
    if fd >= 0 {
        // The counter stays open: the file is never dropped.
        let mut file = std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(fd) });
        file.read_exact(&mut value).unwrap();
    }
    u64::from_ne_bytes(value)
}

/// Makes `n` pairs of objects that all die at once.
#[inline(never)]
fn young(n: usize) -> u64 {
    let mut sum = 0;
    for i in 0..n {
        let small = java::Arc::new(Small { next: None, value: i as u64, count: 1 });
        let medium = java::Arc::new(Medium {
            left: Some(small),
            right: None,
            numbers: [i as u64, 0, 1],
            flag: i.is_multiple_of(3),
        });
        sum += black_box(&medium).numbers[0];
    }
    sum
}

/// Makes `n` pairs of objects of which one in eight is kept in a table of `kept` slots, replacing
/// one kept long ago: the blocks fill with objects of all ages, so their holes are short.
#[inline(never)]
fn kept(n: usize, table: &mut [Option<java::Arc<Medium>>]) -> u64 {
    let mut sum = 0;
    let mut at = 0usize;
    for i in 0..n {
        let small = java::Arc::new(Small { next: None, value: i as u64, count: 2 });
        let medium = java::Arc::new(Medium {
            left: Some(small),
            right: None,
            numbers: [i as u64, 1, 2],
            flag: i.is_multiple_of(5),
        });
        sum += black_box(&medium).numbers[0];
        if i.is_multiple_of(8) {
            at = (at + 7919) % table.len();
            table[at] = Some(medium);
        }
    }
    sum
}

fn main() {
    let _main = java::main_thread();
    let millions: usize = std::env::args().nth(1).map_or(10, |a| a.parse().unwrap());
    let n = millions * 1_000_000;
    let (cycles_fd, instructions_fd) = (counter(0), counter(1));
    let mut table: Vec<Option<java::Arc<Medium>>> = vec![None; 400_000];
    for pattern in ["young", "kept"] {
        let (mut best_cycles, mut best_instructions) = (u64::MAX, u64::MAX);
        let (_, collections_before) = java::gc::statistics();
        for _ in 0..5 {
            let (c, i) = (read_counter(cycles_fd), read_counter(instructions_fd));
            let sum = match pattern {
                "young" => young(n),
                _ => kept(n, &mut table),
            };
            black_box(sum);
            best_cycles = best_cycles.min(read_counter(cycles_fd) - c);
            best_instructions = best_instructions.min(read_counter(instructions_fd) - i);
        }
        let (alive, collections) = java::gc::statistics();
        let allocations = 2.0 * n as f64;
        println!(
            "{pattern:6} {:.2} instructions, {:.2} cycles per allocation; {} collections, {} MB alive",
            best_instructions as f64 / allocations,
            best_cycles as f64 / allocations,
            collections - collections_before,
            alive >> 20,
        );
    }
    let kept = table.iter().flatten().filter(|m| m.left.as_ref().is_some_and(|s| s.count == 2)).count();
    assert!(
        kept > 0
            && table
                .iter()
                .flatten()
                .all(|m| m.right.is_none() && m.numbers[1] == 1 && m.flag == m.numbers[0].is_multiple_of(5))
    );
    let small = Small { next: None, value: 0, count: 0 };
    black_box((small.next.is_none(), small.value));
}
