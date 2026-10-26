//! The natives of the JDK's internal classes that a translated JDK class calls and that only the
//! JVM can answer: class data sharing (`jdk.internal.misc.CDS`), continuations
//! (`jdk.internal.vm.ContinuationSupport`), the layout of arrays (`jdk.internal.misc.Unsafe`), the
//! wall clock behind `Instant.now()` (`jdk.internal.misc.VM`), the checksums of `java.util.zip`
//! (`CRC32`, `Adler32`) and the processes of `java.lang.ProcessHandleImpl`. Each
//! answers as HotSpot does, or as HotSpot does when run with the flag named, where the default
//! depends on something a translated program does not have (an archive, continuations).

use std::time::{SystemTime, UNIX_EPOCH};

/// `CDS.getCDSConfigStatus()`: the flags of class data sharing (dumping an archive, using one,
/// logging lambda form invokers). A translated program has no archive, so none is set, as on a
/// JVM run with `-Xshare:off`.
pub fn cds_config_status() -> i32 {
    0
}

/// `CDS.getRandomSeedForDumping()`: a seed derived from the JDK's build while an archive is
/// dumped, and 0 otherwise. Never dumping, a translated program gets 0, and
/// `ImmutableCollections` then seeds the iteration order of `Set.of` and `Map.of` from
/// `System.nanoTime()`, as a JVM does.
pub fn cds_random_seed_for_dumping() -> i64 {
    0
}

/// `ContinuationSupport.isSupported0()`: whether the JVM implements continuations, which
/// virtual threads run on. A translated program has none, so `false`, as on a JVM run with
/// `-XX:-VMContinuations`: virtual threads are then bound to a platform thread each.
pub fn continuations_supported() -> bool {
    false
}

/// `Unsafe.arrayBaseOffset(c)` for an array class `c`: the offset in bytes of element 0 from
/// the start of the array object. HotSpot on a 64-bit machine with compressed class pointers
/// (the default) has a 16-byte array header for every element type: an 8-byte mark word, a
/// 4-byte class pointer and the 4-byte length, and `long` and `double` elements are 8-byte
/// aligned there already.
pub fn array_base_offset() -> i64 {
    16
}

/// `Unsafe.arrayIndexScale(c)` for an array class `c` whose element type is `component`, as Java
/// names it (`int`, `java.lang.Object`): the size of one element in bytes. A reference takes 4
/// bytes with compressed object pointers, HotSpot's default below 32 GB of heap.
pub fn array_index_scale(component: &str) -> i32 {
    match component {
        "boolean" | "byte" => 1,
        "short" | "char" => 2,
        "int" | "float" => 4,
        "long" | "double" => 8,
        _ => 4,
    }
}

/// The largest distance in seconds between the clock and the offset that
/// `VM.getNanoTimeAdjustment` answers for: 2^32.
const MAX_ADJUSTMENT_SECONDS: i64 = 1 << 32;

/// `VM.getNanoTimeAdjustment(offsetInSeconds)`: the current time in nanoseconds after
/// `offsetInSeconds` seconds since the Unix epoch, or -1 when the offset is 2^32 seconds or
/// more away from now (the caller then asks again with a closer offset).
pub fn nano_time_adjustment(offset_seconds: i64) -> i64 {
    let (seconds, nanos) = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => (d.as_secs() as i64, d.subsec_nanos() as i64),
        // Before the epoch: whole seconds rounded down, so the nanoseconds stay positive.
        Err(e) => {
            let d = e.duration();
            let (s, n) = (d.as_secs() as i64, d.subsec_nanos() as i64);
            if n == 0 { (-s, 0) } else { (-s - 1, 1_000_000_000 - n) }
        }
    };
    adjustment(seconds, nanos, offset_seconds)
}

/// `VM.getNanoTimeAdjustment` at the time `seconds` and `nanos` after the epoch, as HotSpot's
/// `JVM_GetNanoTimeAdjustment` computes it.
fn adjustment(seconds: i64, nanos: i64, offset_seconds: i64) -> i64 {
    let diff = seconds.wrapping_sub(offset_seconds);
    if diff >= MAX_ADJUSTMENT_SECONDS || diff <= -MAX_ADJUSTMENT_SECONDS {
        return -1;
    }
    diff * 1_000_000_000 + nanos
}

/// The table of zlib's CRC-32 (the reflected polynomial `0xedb88320`), by byte.
const CRC_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
};

/// `CRC32.updateBytes0(crc, b, off, len)`: zlib's `crc32(crc, b + off, len)`, the CRC-32 of the
/// bytes continued from the checksum `crc` (0 for none yet). The caller has checked the range.
pub fn crc32_update_bytes(crc: i32, b: &(impl crate::Source<i8> + ?Sized), off: i32, len: i32) -> i32 {
    let mut c = !(crc as u32);
    for &byte in b.range(off as usize..(off + len) as usize).iter() {
        c = CRC_TABLE[((c ^ byte as u8 as u32) & 0xff) as usize] ^ (c >> 8);
    }
    !c as i32
}

/// `CRC32.update(crc, b)`: the checksum `crc` continued with the low eight bits of `b`.
pub fn crc32_update(crc: i32, b: i32) -> i32 {
    crc32_update_bytes(crc, &[b as i8], 0, 1)
}

/// `CRC32.updateByteBuffer0(crc, address, off, len)`: the checksum continued with the `len` bytes
/// at `address + off`, memory outside the heap (a direct buffer's); `CRC32` has checked that the
/// address is not 0.
pub fn crc32_update_buffer(crc: i32, address: i64, off: i32, len: i32) -> i32 {
    let bytes: Vec<i8> =
        crate::read_native(address + off as i64, len.max(0) as usize).into_iter().map(|b| b as i8).collect();
    crc32_update_bytes(crc, &bytes[..], 0, bytes.len() as i32)
}

/// The modulus of Adler-32, the largest prime below 2^16.
const ADLER_BASE: u32 = 65521;

/// `Adler32.updateBytes(adler, b, off, len)`: zlib's `adler32(adler, b + off, len)`, the Adler-32
/// of the bytes continued from the checksum `adler` (1 for none yet). The caller has checked the
/// range.
pub fn adler32_update_bytes(adler: i32, b: &(impl crate::Source<i8> + ?Sized), off: i32, len: i32) -> i32 {
    adler32(adler, b.range(off as usize..(off + len) as usize).iter().map(|&b| b as u8))
}

/// `Adler32.update(adler, b)`: the checksum `adler` continued with the low eight bits of `b`.
pub fn adler32_update(adler: i32, b: i32) -> i32 {
    adler32(adler, [b as u8])
}

/// `Adler32.updateByteBuffer(adler, address, off, len)`: the checksum continued with the `len`
/// bytes at `address + off`, memory outside the heap (a direct buffer's).
pub fn adler32_update_buffer(adler: i32, address: i64, off: i32, len: i32) -> i32 {
    adler32(adler, crate::read_native(address + off as i64, len.max(0) as usize))
}

/// The Adler-32 `adler` continued with `bytes`: its low half is 1 plus the sum of the bytes, its
/// high half the sum of the low half after each byte, both modulo `ADLER_BASE`.
fn adler32(adler: i32, bytes: impl IntoIterator<Item = u8>) -> i32 {
    let (mut low, mut high) = (adler as u32 & 0xffff, (adler as u32 >> 16) & 0xffff);
    for byte in bytes {
        low = (low + byte as u32) % ADLER_BASE;
        high = (high + low) % ADLER_BASE;
    }
    ((high << 16) | low) as i32
}

/// `ProcessHandleImpl.getCurrentPid0()`: the id of the process.
pub fn current_pid() -> i64 {
    std::process::id() as i64
}

/// `ProcessHandleImpl.isAlive0(pid)`, as the JDK answers it on Linux from `/proc/<pid>/stat`: -1
/// when the process has no such file (it does not exist), 0 when the file's fields cannot be
/// read, and otherwise the time the process started, in milliseconds since the epoch: the boot
/// time (`btime` of `/proc/stat`) plus the start's clock ticks after boot (the 22nd field).
pub fn process_start_time(pid: i64) -> i64 {
    let Ok(stat) = std::fs::read(format!("/proc/{}/stat", pid as i32)) else { return -1 };
    // The JDK reads at most 2047 bytes. The command is between parentheses and may hold anything,
    // so the fields follow the last `)`.
    let stat = String::from_utf8_lossy(&stat[..stat.len().min(2047)]).into_owned();
    let Some(open) = stat.find('(') else { return -1 };
    let Some(close) = stat[open + 1..].rfind(')') else { return -1 };
    // From the state (the 3rd field): the parent (4th), the user and system times (14th and 15th),
    // and the start (22nd), which the JDK all needs.
    let fields: Vec<&str> = stat[open + 1 + close + 1..].split_ascii_whitespace().collect();
    let number = |i: usize| fields.get(i).and_then(|f| f.parse::<u64>().ok());
    let parent = fields.get(1).and_then(|p| p.parse::<i32>().ok());
    let (Some(_), Some(_), Some(_), Some(start)) = (parent, number(11), number(12), number(19)) else {
        return 0;
    };
    boot_time_ms().wrapping_add((start.wrapping_mul(1000) / clock_ticks_per_second()) as i64)
}

/// The kernel's clock ticks per second (`sysconf(_SC_CLK_TCK)`), from the process's auxiliary
/// vector (`AT_CLKTCK`); without it 100, the kernel's `USER_HZ` on every architecture.
fn clock_ticks_per_second() -> u64 {
    const AT_CLKTCK: u64 = 17;
    let auxv = std::fs::read("/proc/self/auxv").unwrap_or_default();
    let word = |c: &[u8]| u64::from_ne_bytes(c.try_into().expect("eight bytes"));
    auxv.as_chunks::<16>()
        .0
        .iter()
        .find(|pair| word(&pair[..8]) == AT_CLKTCK)
        .map(|pair| word(&pair[8..]))
        .filter(|&ticks| ticks > 0)
        .unwrap_or(100)
}

/// The time the system booted, in milliseconds since the epoch: `btime` of `/proc/stat`, in
/// seconds; 0 without it, as the JDK has it.
fn boot_time_ms() -> i64 {
    let stat = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    let btime =
        stat.lines().find_map(|l| l.strip_prefix("btime ")).and_then(|t| t.trim().parse::<i64>().ok());
    btime.unwrap_or(0) * 1000
}

/// `Unsafe.getLoadAverage0(loadavg, nelems)`: the system's load averages over the last 1, 5 and
/// 15 minutes, the first `nelems` of them (`Unsafe` has checked 1 to 3, and the array's length)
/// into `loadavg`, from the C library's `getloadavg`; the number written, or -1 where the system
/// does not tell.
pub fn load_average(loadavg: &mut [f64], nelems: i32) -> i32 {
    let mut la = [0f64; 3];
    // Safety: `la` has room for the three samples that `getloadavg` writes at most.
    let n = unsafe { getloadavg(la.as_mut_ptr(), nelems.clamp(0, 3)) };
    for (slot, sample) in loadavg.iter_mut().zip(la).take(n.max(0) as usize) {
        *slot = sample;
    }
    n
}

/// `Reference.waitForReferencePendingList()`, which the reference handler thread calls before it
/// takes the references that the collector discovered: a translated program's collector
/// (`gc.rs`) traces a referent as any other field and discovers none, so it waits for ever, as
/// on a JVM whose collector never needs the memory.
pub fn wait_for_pending_references() {
    loop {
        std::thread::park();
    }
}

/// `OperatingSystemImpl.getTotalMemorySize0()` and `getFreeMemorySize0()`: the machine's memory
/// and its free memory, `sysconf`'s pages times the page size, as the JDK reads them on Linux.
pub fn os_total_memory() -> i64 {
    sysconf_value(SC_PHYS_PAGES) * sysconf_value(SC_PAGESIZE)
}

pub fn os_free_memory() -> i64 {
    sysconf_value(SC_AVPHYS_PAGES) * sysconf_value(SC_PAGESIZE)
}

/// `OperatingSystemImpl.getTotalSwapSpaceSize0()` and `getFreeSwapSpaceSize0()`: `sysinfo`'s
/// swap in bytes; 0 where it fails.
pub fn os_total_swap() -> i64 {
    system_info().map_or(0, |(total, _)| total)
}

pub fn os_free_swap() -> i64 {
    system_info().map_or(0, |(_, free)| free)
}

/// `OperatingSystemImpl.getCommittedVirtualMemorySize0()`: the process's virtual memory, the
/// `vsize` of `/proc/self/stat` (its 23rd field); -1 where it cannot be read.
pub fn os_committed_virtual_memory() -> i64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    // The fields after the command, which is between parentheses and may hold anything; the
    // state is the 3rd field.
    let fields = stat.rfind(')').map_or("", |close| &stat[close + 1..]);
    fields.split_ascii_whitespace().nth(20).and_then(|v| v.parse::<u64>().ok()).map_or(-1, |v| v as i64)
}

/// `OperatingSystemImpl.getProcessCpuTime0()`: the processor time of the process, its user and
/// system time from `times()`, in nanoseconds counted in whole clock ticks, as the JDK has it.
pub fn os_process_cpu_time() -> i64 {
    let mut tms = [0i64; 4];
    // Safety: `tms` is a `struct tms`, four `clock_t`s.
    unsafe { times(&mut tms) };
    let per_tick = 1_000_000_000 / sysconf_value(SC_CLK_TCK).max(1);
    (tms[0] + tms[1]) * per_tick
}

/// `OperatingSystemImpl.getOpenFileDescriptorCount0()`: the descriptors the process has open,
/// the entries of `/proc/self/fd` but the one that reads it; -1 where it cannot be read.
pub fn os_open_file_descriptors() -> i64 {
    let Ok(dir) = std::fs::read_dir("/proc/self/fd") else { return -1 };
    let numbered =
        dir.filter_map(Result::ok).filter(|e| e.file_name().as_encoded_bytes()[0].is_ascii_digit());
    numbered.count() as i64 - 1
}

/// `OperatingSystemImpl.getMaxFileDescriptorCount0()`: the most descriptors the process may have
/// open, `getrlimit(RLIMIT_NOFILE)`. HotSpot raises its soft limit to the hard one when it starts
/// (`-XX:+MaxFDLimit`, the default on Linux), so this is the hard limit; -1 for none.
pub fn os_max_file_descriptors() -> i64 {
    const RLIMIT_NOFILE: i32 = 7;
    let mut limit = [0u64; 2];
    // Safety: `limit` is a `struct rlimit`, its soft and hard limits.
    match unsafe { getrlimit(RLIMIT_NOFILE, &mut limit) } {
        0 => limit[1] as i64,
        _ => -1,
    }
}

/// `OperatingSystemImpl.getHostOnlineCpuCount0()`: the processors online,
/// `sysconf(_SC_NPROCESSORS_ONLN)`, and 1 where it does not tell.
pub fn os_online_processors() -> i32 {
    (sysconf_value(SC_NPROCESSORS_ONLN) as i32).max(1)
}

/// Linux's names of the values of `sysconf`.
const SC_CLK_TCK: i32 = 2;
const SC_PAGESIZE: i32 = 30;
const SC_NPROCESSORS_ONLN: i32 = 84;
const SC_PHYS_PAGES: i32 = 85;
const SC_AVPHYS_PAGES: i32 = 86;

fn sysconf_value(name: i32) -> i64 {
    // Safety: `sysconf` reads a value of the system and has no other effect.
    #[allow(clippy::useless_conversion)]
    i64::from(unsafe { sysconf(name) })
}

/// `sysinfo`'s total and free swap, in bytes.
fn system_info() -> Option<(i64, i64)> {
    // `struct sysinfo` on 64-bit Linux: the uptime, three loads, six sizes (the total and free
    // swap 9th and 10th words), the processes, two more sizes, then the unit of the sizes, an
    // `int` at byte 104; 112 bytes in all.
    let mut buf = [0u64; 16];
    // Safety: `buf` is larger than a `struct sysinfo`.
    if unsafe { sysinfo(buf.as_mut_ptr().cast()) } != 0 {
        return None;
    }
    let unit = u32::from_ne_bytes(buf[13].to_ne_bytes()[..4].try_into().expect("four bytes")).max(1) as u64;
    Some((buf[8].wrapping_mul(unit) as i64, buf[9].wrapping_mul(unit) as i64))
}

unsafe extern "C" {
    fn getloadavg(loadavg: *mut f64, nelem: i32) -> i32;
    fn sysconf(name: i32) -> std::ffi::c_long;
    fn sysinfo(info: *mut u8) -> i32;
    fn times(buf: *mut [i64; 4]) -> std::ffi::c_long;
    fn getrlimit(resource: i32, limit: *mut [u64; 2]) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_class_data_sharing_and_no_continuations() {
        assert_eq!(cds_config_status(), 0);
        assert_eq!(cds_random_seed_for_dumping(), 0);
        assert!(!continuations_supported());
    }

    #[test]
    fn array_layout_is_hotspots_default() {
        // What `Unsafe.ARRAY_*_BASE_OFFSET` and `ARRAY_*_INDEX_SCALE` are on a 64-bit HotSpot.
        assert_eq!(array_base_offset(), 16);
        let scales: Vec<i32> = ["boolean", "byte", "short", "char", "int", "float", "long", "double"]
            .iter()
            .map(|c| array_index_scale(c))
            .collect();
        assert_eq!(scales, [1, 1, 2, 2, 4, 4, 8, 8]);
        assert_eq!(array_index_scale("java.lang.Object"), 4);
        assert_eq!(array_index_scale("java.util.concurrent.ForkJoinTask"), 4);
        // `ArraysSupport` takes the log of each scale and fails on anything but a power of two.
        assert!(scales.iter().all(|s| s.count_ones() == 1));
    }

    #[test]
    fn nano_time_adjustment_is_nanoseconds_after_the_offset() {
        assert_eq!(adjustment(1_000, 5, 1_000), 5);
        assert_eq!(adjustment(1_002, 5, 1_000), 2_000_000_005);
        assert_eq!(adjustment(998, 5, 1_000), -1_999_999_995);
        // Within 2^32 seconds of the offset, on either side, and -1 from there on.
        let far = MAX_ADJUSTMENT_SECONDS;
        assert_eq!(adjustment(far - 1, 7, 0), (far - 1) * 1_000_000_000 + 7);
        assert_eq!(adjustment(far, 7, 0), -1);
        assert_eq!(adjustment(0, 7, far), -1);
        assert_eq!(adjustment(0, 7, far - 1), -(far - 1) * 1_000_000_000 + 7);
        assert_eq!(adjustment(0, 0, i64::MIN), -1);
        assert_eq!(adjustment(0, 0, i64::MAX), -1);
    }

    #[test]
    fn nano_time_adjustment_follows_the_clock() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        let a = nano_time_adjustment(now);
        // Within a minute of the offset, and moving forward.
        assert!((0..60_000_000_000).contains(&a), "{a}");
        assert!(nano_time_adjustment(now) >= a);
        assert_eq!(nano_time_adjustment(now - MAX_ADJUSTMENT_SECONDS - 10), -1);
        assert!(nano_time_adjustment(0) > 0);
    }

    #[test]
    fn crc32_is_zlibs() {
        let bytes: Vec<i8> = b"123456789".iter().map(|&b| b as i8).collect();
        // The standard check value of CRC-32, and Java's `new CRC32().update(b)` gives it too.
        assert_eq!(crc32_update_bytes(0, &bytes, 0, 9), 0xcbf4_3926_u32 as i32);
        let half = crc32_update_bytes(0, &bytes, 0, 4);
        assert_eq!(crc32_update_bytes(half, &bytes, 4, 5), 0xcbf4_3926_u32 as i32);
        let one_by_one = bytes.iter().fold(0, |crc, &b| crc32_update(crc, b as i32));
        assert_eq!(one_by_one, 0xcbf4_3926_u32 as i32);
        assert_eq!(crc32_update_bytes(7, &bytes, 3, 0), 7);
        // Only the low eight bits of the `int` count.
        assert_eq!(crc32_update(0, 0x161), crc32_update(0, 0x61));
        assert_eq!(crc32_update(0, b'a' as i32), 0xe8b7_be43_u32 as i32);
    }

    #[test]
    fn adler32_is_zlibs() {
        let bytes: Vec<i8> = b"Wikipedia".iter().map(|&b| b as i8).collect();
        assert_eq!(adler32_update_bytes(1, &bytes, 0, 9), 0x11e6_0398);
        let half = adler32_update_bytes(1, &bytes, 0, 4);
        assert_eq!(adler32_update_bytes(half, &bytes, 4, 5), 0x11e6_0398);
        let one_by_one = bytes.iter().fold(1, |adler, &b| adler32_update(adler, b as i32));
        assert_eq!(one_by_one, 0x11e6_0398);
        assert_eq!(adler32_update_bytes(7, &bytes, 3, 0), 7);
        assert_eq!(adler32_update(1, 0x161), adler32_update(1, 0x61));
        // The sums wrap at the modulus: 0xff bytes past it.
        let ff = vec![-1i8; 6000];
        assert_eq!(adler32_update_bytes(1, &ff, 0, 6000), adler32(1, std::iter::repeat_n(0xff, 6000)));
        assert_eq!(adler32(1, std::iter::repeat_n(0xff, 6000)) as u32 & 0xffff, (1 + 6000 * 255) % 65521);
    }

    #[test]
    fn the_current_process_is_alive_since_it_started() {
        assert_eq!(current_pid(), std::process::id() as i64);
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
        let started = process_start_time(current_pid());
        // Started in the last day, and not after now (the boot time has whole seconds).
        assert!((now - 86_400_000..=now + 1000).contains(&started), "{started} {now}");
        assert_eq!(process_start_time(-5), -1);
    }

    #[test]
    fn the_system_answers_its_counts() {
        let mut la = [-1.0; 3];
        assert_eq!(load_average(&mut la[..2], 2), 2);
        assert!(la[0] >= 0.0 && la[1] >= 0.0 && la[2] == -1.0, "{la:?}");
        assert!(os_total_memory() > os_free_memory() && os_free_memory() > 0);
        assert!(os_total_swap() >= os_free_swap() && os_free_swap() >= 0);
        assert!(os_committed_virtual_memory() > 0);
        assert!(os_process_cpu_time() >= 0);
        // Standard input, output and error at least.
        assert!(os_open_file_descriptors() >= 3);
        assert!(os_max_file_descriptors() >= os_open_file_descriptors() || os_max_file_descriptors() == -1);
        assert!(os_online_processors() >= 1);
    }
}
