//! The `native` methods of `java.lang` that a translated program reaches: `System.arraycopy`,
//! the bit conversions of `Double` and `Float`, identity hash codes, clocks and the few system
//! properties that are the same on every run. They behave as HotSpot's do.

use crate::array::Array;
use crate::throw;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// `System.arraycopy(src, src_pos, dest, dest_pos, length)` between shared arrays. When `src`
/// and `dest` are the same array the copy behaves as if it went through a temporary array.
pub fn arraycopy<T: Clone>(src: &Array<T>, src_pos: i32, dest: &Array<T>, dest_pos: i32, length: i32) {
    if Rc::ptr_eq(src, dest) {
        copy_within(&mut dest.borrow_mut(), src_pos, dest_pos, length);
    } else {
        copy_slice(&src.borrow(), src_pos, &mut dest.borrow_mut(), dest_pos, length);
    }
}

/// `System.arraycopy` from one array to a different one, either of which may be a plain array.
pub fn copy_slice<T: Clone>(src: &[T], src_pos: i32, dest: &mut [T], dest_pos: i32, length: i32) {
    let (from, to, n) = check_copy::<T>(src.len(), src_pos, dest.len(), dest_pos, length);
    dest[to..to + n].clone_from_slice(&src[from..from + n]);
}

/// `System.arraycopy(a, src_pos, a, dest_pos, length)` within one array: overlapping ranges are
/// copied as if through a temporary array.
pub fn copy_within<T: Clone>(a: &mut [T], src_pos: i32, dest_pos: i32, length: i32) {
    let (from, to, n) = check_copy::<T>(a.len(), src_pos, a.len(), dest_pos, length);
    if from < to {
        for k in (0..n).rev() {
            a[to + k] = a[from + k].clone();
        }
    } else {
        for k in 0..n {
            a[to + k] = a[from + k].clone();
        }
    }
}

/// Checks the positions of a copy before anything is copied, throwing what HotSpot throws, and
/// returns them as `usize`.
fn check_copy<T>(
    src_len: usize,
    src_pos: i32,
    dest_len: usize,
    dest_pos: i32,
    length: i32,
) -> (usize, usize, usize) {
    let name = element_name::<T>();
    let fail = |message: String| -> ! { throw("java.lang.ArrayIndexOutOfBoundsException", &message) };
    if src_pos < 0 {
        fail(format!("arraycopy: source index {src_pos} out of bounds for {name}[{src_len}]"));
    }
    if dest_pos < 0 {
        fail(format!("arraycopy: destination index {dest_pos} out of bounds for {name}[{dest_len}]"));
    }
    if length < 0 {
        fail(format!("arraycopy: length {length} is negative"));
    }
    let (from, to, n) = (src_pos as usize, dest_pos as usize, length as usize);
    if from + n > src_len {
        fail(format!("arraycopy: last source index {} out of bounds for {name}[{src_len}]", from + n));
    }
    if to + n > dest_len {
        fail(format!("arraycopy: last destination index {} out of bounds for {name}[{dest_len}]", to + n));
    }
    (from, to, n)
}

/// How HotSpot names an array's element type in an `arraycopy` message.
fn element_name<T>() -> &'static str {
    match std::any::type_name::<T>() {
        "i32" => "int",
        "i64" => "long",
        "f64" => "double",
        "f32" => "float",
        "i8" => "byte",
        "i16" => "short",
        "u16" => "char",
        "bool" => "boolean",
        _ => "object array",
    }
}

/// `Double.doubleToLongBits`: every NaN gives the bits of the canonical NaN, `0x7ff8000000000000`.
pub fn double_to_long_bits(d: f64) -> i64 {
    if d.is_nan() { 0x7ff8_0000_0000_0000 } else { d.to_bits() as i64 }
}

/// `Double.doubleToRawLongBits`.
pub fn double_to_raw_long_bits(d: f64) -> i64 {
    d.to_bits() as i64
}

/// `Double.longBitsToDouble`.
pub fn long_bits_to_double(bits: i64) -> f64 {
    f64::from_bits(bits as u64)
}

/// `Float.floatToIntBits`: every NaN gives the bits of the canonical NaN, `0x7fc00000`.
pub fn float_to_int_bits(f: f32) -> i32 {
    if f.is_nan() { 0x7fc0_0000 } else { f.to_bits() as i32 }
}

/// `Float.floatToRawIntBits`.
pub fn float_to_raw_int_bits(f: f32) -> i32 {
    f.to_bits() as i32
}

/// `Float.intBitsToFloat`.
pub fn int_bits_to_float(bits: i32) -> f32 {
    f32::from_bits(bits as u32)
}

/// `System.identityHashCode(o)` and an `Object.hashCode()` that no class overrides, for an
/// object behind an `Rc`.
///
/// HotSpot draws the hash from a thread-local random generator the first time it is asked for
/// and stores it in the object header, so no program can print the same values as Java and none
/// can depend on them. This one is derived from the object's address, which an `Rc` never
/// changes: it is the same for the object's whole life, different for objects alive at the same
/// time in all but rare collisions, and, like HotSpot's, a positive 31-bit number. The address
/// is mixed (multiplied by an odd constant, high bits taken) so that neighbouring objects do not
/// get neighbouring hashes.
pub fn identity_hash<T: ?Sized>(rc: &Rc<T>) -> i32 {
    address_hash(Rc::as_ptr(rc))
}

/// The identity hash (see `identity_hash`) of the object at `address`, which an `Rc` or a borrow
/// of the value inside one gives.
pub fn address_hash<T: ?Sized>(address: *const T) -> i32 {
    let address = address as *const u8 as usize as u64;
    let mixed = address.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let hash = (mixed >> 33) as i32;
    if hash == 0 { 1 } else { hash }
}

/// What `Object.toString()` makes for an object of a class that does not override it: the
/// class's binary name, `@` and the object's hash code in hexadecimal.
pub fn default_text(class: &str, hash: i32) -> String {
    format!("{class}@{hash:x}")
}

/// `System.currentTimeMillis()`: milliseconds since the Unix epoch.
pub fn current_time_millis() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_millis() as i64,
        Err(e) => -(e.duration().as_millis() as i64),
    }
}

/// `System.nanoTime()`: nanoseconds on a monotonic clock from an arbitrary origin (here the
/// first call), which is all Java promises.
pub fn nano_time() -> i64 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    let origin = *ORIGIN.get_or_init(Instant::now);
    origin.elapsed().as_nanos() as i64
}

/// `Runtime.getRuntime().availableProcessors()`: like Java, the processors this process may run
/// on (CPU affinity and cgroup quotas count), at least one.
pub fn available_processors() -> i32 {
    std::thread::available_parallelism().map_or(1, |n| n.get() as i32)
}

/// `System.lineSeparator()` and `System.getProperty("line.separator")`; the translator targets
/// Unix-like systems, where it is `"\n"`.
pub fn line_separator() -> String {
    "\n".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::from_vec;

    #[test]
    fn arraycopy_overlapping_moves_as_through_a_temporary() {
        let a = from_vec(vec![1, 2, 3, 4, 5]);
        arraycopy(&a, 0, &a, 1, 4);
        assert_eq!(*a.borrow(), [1, 1, 2, 3, 4]);
        arraycopy(&a, 1, &a, 0, 4);
        assert_eq!(*a.borrow(), [1, 2, 3, 4, 4]);
        let mut v = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        copy_within(&mut v, 1, 0, 2);
        assert_eq!(v, ["b", "c", "c"]);
    }

    #[test]
    fn arraycopy_between_arrays() {
        let src = from_vec(vec![1.5, 2.5]);
        let dest = from_vec(vec![0.0; 4]);
        arraycopy(&src, 0, &dest, 2, 2);
        assert_eq!(*dest.borrow(), [0.0, 0.0, 1.5, 2.5]);
        let mut d = [0i64; 3];
        copy_slice(&[7, 8, 9], 1, &mut d, 0, 2);
        assert_eq!(d, [8, 9, 0]);
        copy_slice(&[1, 2], 2, &mut d, 3, 0);
    }

    #[test]
    fn bit_conversions_match_java() {
        // Expected values from Java: Double.doubleToLongBits and friends.
        let negative_nan = f64::from_bits(0xfff8_0000_0000_0000);
        assert_eq!(double_to_long_bits(negative_nan), 0x7ff8_0000_0000_0000);
        assert_eq!(double_to_raw_long_bits(negative_nan), 0xfff8_0000_0000_0000_u64 as i64);
        assert_eq!(double_to_long_bits(-0.0), i64::MIN);
        assert_eq!(double_to_long_bits(1.0), 0x3ff0_0000_0000_0000);
        assert_eq!(long_bits_to_double(1).to_bits(), 1);
        assert_eq!(long_bits_to_double(0x7ff0_0000_0000_0001).to_bits(), 0x7ff0_0000_0000_0001);
        assert_eq!(float_to_int_bits(f32::from_bits(0xffc0_0001)), 0x7fc0_0000);
        assert_eq!(float_to_raw_int_bits(f32::from_bits(0xffc0_0001)), 0xffc0_0001_u32 as i32);
        assert_eq!(float_to_int_bits(1.0), 0x3f80_0000);
        assert_eq!(int_bits_to_float(0x7f80_0000), f32::INFINITY);
    }

    #[test]
    fn identity_hash_is_stable_and_distinct() {
        let a = Rc::new(std::cell::RefCell::new(5));
        let b = Rc::new(std::cell::RefCell::new(5));
        assert_eq!(identity_hash(&a), identity_hash(&a.clone()));
        assert_ne!(identity_hash(&a), identity_hash(&b));
        assert!(identity_hash(&a) > 0 && identity_hash(&b) > 0);
        let s: Rc<str> = Rc::from("x");
        assert!(identity_hash(&s) > 0);
    }

    #[test]
    fn clocks_advance() {
        let t0 = nano_time();
        let m0 = current_time_millis();
        assert!(nano_time() >= t0);
        assert!(m0 > 1_600_000_000_000);
        assert!(available_processors() >= 1);
    }
}
