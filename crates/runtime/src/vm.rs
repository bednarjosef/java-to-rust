//! The natives of the JDK's internal classes that a translated JDK class calls and that only the
//! JVM can answer: class data sharing (`jdk.internal.misc.CDS`), continuations
//! (`jdk.internal.vm.ContinuationSupport`), the layout of arrays (`jdk.internal.misc.Unsafe`), the
//! wall clock behind `Instant.now()` (`jdk.internal.misc.VM`) and `java.util.zip.CRC32`. Each
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
}
