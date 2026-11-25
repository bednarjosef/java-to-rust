//! Times the collector's marking on a heap shaped like a large server's: about 300 MB alive, in
//! many tables of entries whose values point to each other at random, beside pointer-free
//! arrays, while short-lived garbage is churned.
//!
//!     cargo run --release -p java-to-rust-runtime --example marking [alive-mb] [collections]
//!
//! It prints the median and fastest pause of the collections it runs (`gc::collect_now`, each a
//! whole collection: stopping, marking, sweeping), the processor time they took, all markers',
//! and the cycles and instructions of the collecting thread, which with `JAVA_GC_THREADS=1` are
//! all of the marking's and vary less than times on a busy machine. Then it checks that the
//! tables came through intact. `JAVA_GC_LOG=1` adds the runtime's line per collection.

use java_runtime as java;
use std::hint::black_box;
use std::time::Instant;

#[global_allocator]
static ALLOCATOR: java::Allocator = java::Allocator;

/// A value of a table: a few references to other values, and numbers.
struct Value {
    next: Option<java::Arc<Value>>,
    other: Option<java::Arc<Value>>,
    numbers: [u64; 4],
}

/// An entry of a table's bucket chain.
struct Entry {
    key: u64,
    value: java::Arc<Value>,
    next: Option<java::Arc<Entry>>,
}

/// A table: buckets of chained entries, and a pointer-free array beside it.
struct Table {
    buckets: Vec<Option<java::Arc<Entry>>>,
    numbers: Vec<u32>,
    first: Option<java::Arc<Value>>,
}

struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
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

fn process_seconds() -> f64 {
    let mut t = std::mem::MaybeUninit::<[i64; 2]>::uninit();
    unsafe extern "C" {
        fn clock_gettime(clock: i32, time: *mut [i64; 2]) -> i32;
    }
    // CLOCK_PROCESS_CPUTIME_ID
    unsafe { clock_gettime(2, t.as_mut_ptr()) };
    let t = unsafe { t.assume_init() };
    t[0] as f64 + t[1] as f64 * 1e-9
}

#[inline(never)]
fn build(alive_mb: usize, random: &mut Random) -> Vec<java::Arc<Table>> {
    // Each table holds 1024 entries and values (about 76 KB) and 16 KB of numbers.
    let tables = alive_mb * 1024 / 92;
    let mut all: Vec<java::Arc<Value>> = Vec::new();
    let mut out = Vec::with_capacity(tables);
    for _ in 0..tables {
        let mut buckets: Vec<Option<java::Arc<Entry>>> = vec![None; 512];
        let mut first = None;
        for i in 0..1024u64 {
            // Values point to their neighbour and to one made earlier anywhere in the heap.
            let other =
                if all.is_empty() { None } else { Some(all[random.next() as usize % all.len()].clone()) };
            let value = java::Arc::new(Value { next: first.take(), other, numbers: [i; 4] });
            if random.next().is_multiple_of(16) {
                all.push(value.clone());
            }
            first = Some(value.clone());
            let key = random.next();
            let bucket = (key % 512) as usize;
            let entry = java::Arc::new(Entry { key, value, next: buckets[bucket].take() });
            buckets[bucket] = Some(entry);
        }
        let numbers = (0..4096).map(|_| random.next() as u32).collect();
        out.push(java::Arc::new(Table { buckets, numbers, first }));
    }
    out
}

/// Walks every table, checking that what the collections kept is intact: each table's 1024
/// entries, whose values are its chain of 1024 values, and its numbers.
#[inline(never)]
fn check(tables: &[java::Arc<Table>]) {
    for table in tables {
        let mut entries = 0;
        for (i, bucket) in table.buckets.iter().enumerate() {
            let mut entry = bucket.as_ref();
            while let Some(e) = entry {
                assert_eq!(e.key % 512, i as u64);
                assert!(e.value.numbers[0] < 1024);
                entries += 1;
                entry = e.next.as_ref();
            }
        }
        assert_eq!(entries, 1024);
        let (mut values, mut value) = (0u64, table.first.as_ref());
        while let Some(v) = value {
            assert_eq!(v.numbers, [1023 - values; 4]);
            assert!(v.other.as_ref().is_none_or(|o| o.numbers[0] < 1024));
            values += 1;
            value = v.next.as_ref();
        }
        assert_eq!(values, 1024);
        assert_eq!(table.numbers.len(), 4096);
    }
}

/// Makes `n` short-lived values.
#[inline(never)]
fn churn(n: usize, random: &mut Random) -> u64 {
    let mut sum = 0;
    let mut last: Option<java::Arc<Value>> = None;
    for i in 0..n {
        let v = java::Arc::new(Value { next: last.take(), other: None, numbers: [random.next(); 4] });
        sum += v.numbers[0] & 1;
        if i % 8 != 0 {
            last = Some(v);
        }
    }
    sum
}

fn main() {
    let _main = java::main_thread();
    let mut args = std::env::args().skip(1);
    let alive_mb: usize = args.next().map_or(300, |a| a.parse().unwrap());
    let collections: usize = args.next().map_or(15, |a| a.parse().unwrap());
    let mut random = Random(0x9E37_79B9_7F4A_7C15);
    let tables = build(alive_mb, &mut random);
    black_box(churn(4_000_000, &mut random));
    let (alive, _) = java::gc::statistics();
    let mut pauses = Vec::new();
    let mut cpu = 0.0;
    // This thread's own cycles and instructions: with one marker, all the marking's.
    let (cycles_fd, instructions_fd) = (counter(0), counter(1));
    let (mut cycles, mut instructions) = (Vec::new(), Vec::new());
    for _ in 0..collections {
        black_box(churn(200_000, &mut random));
        let (c, i) = (read_counter(cycles_fd), read_counter(instructions_fd));
        let (started, before) = (Instant::now(), process_seconds());
        java::gc::collect_now();
        pauses.push(started.elapsed().as_secs_f64() * 1000.0);
        cpu += process_seconds() - before;
        cycles.push(read_counter(cycles_fd) - c);
        instructions.push(read_counter(instructions_fd) - i);
    }
    cycles.sort();
    instructions.sort();
    let (after, _) = java::gc::statistics();
    pauses.sort_by(f64::total_cmp);
    println!(
        "alive {} MB (before {} MB), {} collections: median {:.2} ms, fastest {:.2} ms, processor {:.1} ms per collection; this thread {:.1} M cycles, {:.1} M instructions (median), fewest cycles {:.1} M",
        after >> 20,
        alive >> 20,
        collections,
        pauses[pauses.len() / 2],
        pauses[0],
        cpu * 1000.0 / collections as f64,
        cycles[cycles.len() / 2] as f64 / 1e6,
        instructions[instructions.len() / 2] as f64 / 1e6,
        cycles[0] as f64 / 1e6,
    );
    check(&tables);
}
