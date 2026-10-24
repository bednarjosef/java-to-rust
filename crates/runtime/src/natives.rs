//! The `native` methods of `java.lang` that a translated program reaches: `System.arraycopy`,
//! the bit conversions of `Double` and `Float`, identity hash codes, clocks and the few system
//! properties that are the same on every run. They behave as HotSpot's do.

use crate::array::Elements;
use crate::{Exception, Source, Target, throw};
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// `System.arraycopy(src, src_pos, dest, dest_pos, length)` between shared arrays. When `src`
/// and `dest` are the same array the copy behaves as if it went through a temporary array. Into a
/// covariant array (`crate::ElementClass`) of another class than `src`'s, each element is checked
/// as it is stored.
pub fn arraycopy<T: Clone, A: Elements<T>>(src: &A, src_pos: i32, dest: &A, dest_pos: i32, length: i32) {
    if let Some(class) = checked_destination(src, dest) {
        return checked_copy(src, src_pos, dest, dest_pos, length, class)
            .unwrap_or_else(|e| crate::throw_exception(e));
    }
    let (from, to, n) = check_copy::<T>(src.length(), src_pos, dest.length(), dest_pos, length);
    if src.address() == dest.address() {
        dest.copy_within(from, to, n);
    } else {
        src.reading(from..from + n, |items| dest.store_from(to, items));
    }
}

/// The class of `dest` where it is a covariant array that a copy from `src` must check: `src` is
/// not of the same class.
fn checked_destination<'a, T, A: Elements<T>>(src: &A, dest: &'a A) -> Option<&'a crate::ElementClass<T>> {
    let class = dest.element_class()?;
    let same = src.element_class().is_some_and(|s| s.element == class.element);
    (!same).then_some(class)
}

/// `arraycopy` into a covariant array of another class than `src`'s: the elements up to the
/// first that `dest` cannot hold are stored, and that one throws HotSpot's
/// `ArrayStoreException`.
fn checked_copy<T: Clone, A: Elements<T>>(
    src: &A,
    src_pos: i32,
    dest: &A,
    dest_pos: i32,
    length: i32,
    class: &crate::ElementClass<T>,
) -> Result<(), Exception> {
    let (from, to, n) = copy_bounds::<T>(src.length(), src_pos, dest.length(), dest_pos, length)
        .map_err(|message| Exception::new("java.lang.ArrayIndexOutOfBoundsException", message))?;
    let items: Vec<T> = src.reading(from..from + n, <[T]>::to_vec);
    let fits = items.iter().take_while(|v| class.refuses(v).is_none()).count();
    dest.store_from(to, &items[..fits]);
    if fits < n {
        let source = src.element_class().map_or("java.lang.Object", |c| c.element);
        let message = format!(
            "arraycopy: element type mismatch: can not cast one of the elements of {source}[] to the type of the destination array, {}",
            class.element
        );
        return Err(Exception::new("java.lang.ArrayStoreException", message));
    }
    Ok(())
}

/// `System.arraycopy` from one array to a different one, either of which may be a plain array:
/// only the elements copied are read and stored.
pub fn copy_slice<T: Clone>(
    src: &(impl Source<T> + ?Sized),
    src_pos: i32,
    dest: &mut (impl Target<T> + ?Sized),
    dest_pos: i32,
    length: i32,
) {
    let (from, to, n) = check_copy::<T>(src.length(), src_pos, dest.length(), dest_pos, length);
    dest.store(to, &src.range(from..from + n));
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
    match copy_bounds::<T>(src_len, src_pos, dest_len, dest_pos, length) {
        Ok(bounds) => bounds,
        Err(message) => throw("java.lang.ArrayIndexOutOfBoundsException", &message),
    }
}

/// The positions of a copy as `usize`, or the message of the `ArrayIndexOutOfBoundsException`
/// that HotSpot throws for them.
pub(crate) fn copy_bounds<T>(
    src_len: usize,
    src_pos: i32,
    dest_len: usize,
    dest_pos: i32,
    length: i32,
) -> Result<(usize, usize, usize), String> {
    let name = element_name::<T>();
    if src_pos < 0 {
        return Err(format!("arraycopy: source index {src_pos} out of bounds for {name}[{src_len}]"));
    }
    if dest_pos < 0 {
        return Err(format!("arraycopy: destination index {dest_pos} out of bounds for {name}[{dest_len}]"));
    }
    if length < 0 {
        return Err(format!("arraycopy: length {length} is negative"));
    }
    let (from, to, n) = (src_pos as usize, dest_pos as usize, length as usize);
    if from + n > src_len {
        return Err(format!("arraycopy: last source index {} out of bounds for {name}[{src_len}]", from + n));
    }
    if to + n > dest_len {
        return Err(format!(
            "arraycopy: last destination index {} out of bounds for {name}[{dest_len}]",
            to + n
        ));
    }
    Ok((from, to, n))
}

/// `copy_bounds` as the failure of a checked form.
fn try_bounds<T>(
    src_len: usize,
    src_pos: i32,
    dest_len: usize,
    dest_pos: i32,
    length: i32,
) -> Result<(), Exception> {
    copy_bounds::<T>(src_len, src_pos, dest_len, dest_pos, length)
        .map(|_| ())
        .map_err(|message| Exception::new("java.lang.ArrayIndexOutOfBoundsException", message))
}

/// `arraycopy`, where a `catch` or a `finally` sees its `ArrayIndexOutOfBoundsException`: the
/// failure is returned, to go on as a thrown exception does, and nothing is copied.
pub fn try_arraycopy<T: Clone, A: Elements<T>>(
    src: &A,
    src_pos: i32,
    dest: &A,
    dest_pos: i32,
    length: i32,
) -> Result<(), Exception> {
    if let Some(class) = checked_destination(src, dest) {
        return checked_copy(src, src_pos, dest, dest_pos, length, class);
    }
    let (src_len, dest_len) = (src.length(), dest.length());
    try_bounds::<T>(src_len, src_pos, dest_len, dest_pos, length)?;
    arraycopy(src, src_pos, dest, dest_pos, length);
    Ok(())
}

/// `arraycopy` from an array of `T` into one of `U`, as the copy of references between two
/// arrays of different types. `convert` gives each element as a `U`, or the message of the
/// `ArrayStoreException` that HotSpot throws for it. The elements before the failing one are
/// stored and the rest are not, as HotSpot stores them.
pub fn try_arraycopy_with<T: Clone, U: Clone, S: Elements<T>, D: Elements<U>>(
    src: &S,
    src_pos: i32,
    dest: &D,
    dest_pos: i32,
    length: i32,
    convert: impl Fn(&T) -> Result<U, &'static str>,
) -> Result<(), Exception> {
    let (from, to, n) = copy_bounds::<T>(src.length(), src_pos, dest.length(), dest_pos, length)
        .map_err(|message| Exception::new("java.lang.ArrayIndexOutOfBoundsException", message))?;
    let (stored, failure) = src.reading(from..from + n, |items| crate::copy_refs::converted(items, &convert));
    dest.store_from(to, &stored);
    match failure {
        Some(message) => Err(Exception::new("java.lang.ArrayStoreException", message)),
        None => Ok(()),
    }
}

/// What an `Object` is to `System.arraycopy`: not an array (with its class's binary name), an
/// array of a primitive type (with the type's name), or an array of references.
#[derive(Clone, Copy)]
pub enum CopyKind {
    NotArray(&'static str),
    Primitive(&'static str),
    References,
}

/// The `ArrayStoreException` that HotSpot throws for `System.arraycopy` between objects of kinds
/// `src` and `dest` that cannot be copied: one is not an array, or the element types differ and
/// one of them is primitive.
pub fn arraycopy_mismatch(src: CopyKind, dest: CopyKind) -> Exception {
    use CopyKind::*;
    let mismatch = "arraycopy: type mismatch: can not copy";
    let message = match (src, dest) {
        (NotArray(name), _) => format!("arraycopy: source type {name} is not an array"),
        (_, NotArray(name)) => format!("arraycopy: destination type {name} is not an array"),
        (Primitive(a), Primitive(b)) => format!("{mismatch} {a}[] into {b}[]"),
        (Primitive(a), References) => format!("{mismatch} {a}[] into object array[]"),
        (References, Primitive(b)) => format!("{mismatch} object array[] into {b}[]"),
        (References, References) => format!("{mismatch} object array[] into object array[]"),
    };
    Exception::new("java.lang.ArrayStoreException", message)
}

/// `copy_slice`, where the program observes the exception (see `try_arraycopy`).
pub fn try_copy_slice<T: Clone>(
    src: &(impl Source<T> + ?Sized),
    src_pos: i32,
    dest: &mut (impl Target<T> + ?Sized),
    dest_pos: i32,
    length: i32,
) -> Result<(), Exception> {
    try_bounds::<T>(src.length(), src_pos, dest.length(), dest_pos, length)?;
    copy_slice(src, src_pos, dest, dest_pos, length);
    Ok(())
}

/// `copy_within`, where the program observes the exception (see `try_arraycopy`).
pub fn try_copy_within<T: Clone>(
    a: &mut [T],
    src_pos: i32,
    dest_pos: i32,
    length: i32,
) -> Result<(), Exception> {
    try_bounds::<T>(a.len(), src_pos, a.len(), dest_pos, length)?;
    copy_within(a, src_pos, dest_pos, length);
    Ok(())
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
/// object behind an `Rc` or an `Arc`.
///
/// HotSpot draws the hash from a thread-local random generator the first time it is asked for
/// and stores it in the object header, so no program can print the same values as Java and none
/// can depend on them. This one is derived from the object's address, which an `Rc` never
/// changes: it is the same for the object's whole life, different for objects alive at the same
/// time in all but rare collisions, and, like HotSpot's, a positive 31-bit number. The address
/// is mixed (multiplied by an odd constant, high bits taken) so that neighbouring objects do not
/// get neighbouring hashes.
pub fn identity_hash<H: crate::Handle>(handle: &H) -> i32 {
    address_hash(handle.address())
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
pub fn default_text(class: &str, hash: i32) -> crate::JString {
    format!("{class}@{hash:x}").into()
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
pub fn line_separator() -> crate::JString {
    "\n".into()
}

/// A primitive element of an array as the bytes the JVM keeps it in: little-endian, as on the
/// machines the translation runs on, a `boolean` as one byte.
pub trait RawBytes: Copy {
    const SIZE: usize;
    fn put(self, out: &mut [u8]);
    fn get(bytes: &[u8]) -> Self;
}

macro_rules! raw_bytes {
    ($($t:ty),*) => {$(
        impl RawBytes for $t {
            const SIZE: usize = std::mem::size_of::<$t>();
            fn put(self, out: &mut [u8]) {
                out.copy_from_slice(&self.to_le_bytes());
            }
            fn get(bytes: &[u8]) -> Self {
                <$t>::from_le_bytes(bytes.try_into().expect("an element's bytes"))
            }
        }
    )*};
}
raw_bytes!(i8, i16, u16, i32, i64, f32, f64);

impl RawBytes for bool {
    const SIZE: usize = 1;
    fn put(self, out: &mut [u8]) {
        out[0] = u8::from(self);
    }
    fn get(bytes: &[u8]) -> Self {
        bytes[0] != 0
    }
}

/// Ends the program at an `Unsafe` access of memory that the translation does not have, such as
/// a copy from an address outside the heap.
pub fn unsupported_memory(what: &'static str) -> ! {
    crate::exception::die(format!("{what}, which the translation does not support"))
}

/// The range of bytes `offset..offset + bytes` of an array of `len` elements of `size` bytes, from
/// its first element, where `offset` is from the start of the array object as `Unsafe` counts
/// it. A range outside the elements ends the program: the JVM would read or write other memory.
fn byte_range(len: usize, size: usize, offset: i64, bytes: i64) -> std::ops::Range<usize> {
    let start = offset - crate::array_base_offset();
    let end = start.checked_add(bytes);
    match (usize::try_from(start), end.and_then(|e| usize::try_from(e).ok())) {
        (Ok(start), Some(end)) if end <= len * size => start..end,
        _ => unsupported_memory("an Unsafe copy of memory outside an array"),
    }
}

/// The bytes `offset..offset + bytes` of an array of primitives, the offset counted from the start
/// of the array object (`array_base_offset`), as the JVM keeps them.
fn read_bytes<S: RawBytes, A: Elements<S>>(array: &A, offset: i64, bytes: i64) -> Vec<u8> {
    let range = byte_range(array.length(), S::SIZE, offset, bytes);
    let first = range.start / S::SIZE;
    array.reading(first..range.end.div_ceil(S::SIZE), |items| {
        let mut out = Vec::with_capacity(range.len());
        let mut element = vec![0; S::SIZE];
        for (k, item) in items.iter().enumerate() {
            let i = first + k;
            RawBytes::put(*item, &mut element);
            let from = range.start.max(i * S::SIZE) - i * S::SIZE;
            let to = range.end.min((i + 1) * S::SIZE) - i * S::SIZE;
            out.extend_from_slice(&element[from..to]);
        }
        out
    })
}

/// Writes `bytes` into an array of primitives from `offset` on (see `read_bytes`), changing only
/// those bytes of the elements they fall in.
fn write_bytes<D: RawBytes, B: Elements<D>>(array: &B, offset: i64, bytes: &[u8]) {
    let range = byte_range(array.length(), D::SIZE, offset, bytes.len() as i64);
    let first = range.start / D::SIZE;
    array.writing(first..range.end.div_ceil(D::SIZE), |items| {
        let mut element = vec![0; D::SIZE];
        let mut taken = 0;
        for (k, item) in items.iter_mut().enumerate() {
            let i = first + k;
            RawBytes::put(*item, &mut element);
            let from = range.start.max(i * D::SIZE) - i * D::SIZE;
            let to = range.end.min((i + 1) * D::SIZE) - i * D::SIZE;
            element[from..to].copy_from_slice(&bytes[taken..taken + to - from]);
            taken += to - from;
            *item = D::get(&element);
        }
    })
}

/// `Unsafe.copyMemory0` and `copySwapMemory0` between two arrays of primitives (which may be one
/// array, or arrays of different kinds): `bytes` bytes from `src_offset` in `src` to `dest_offset`
/// in `dest`, the offsets counted from the start of each array object (`array_base_offset`), as
/// the JVM copies them. With `swap` above 1, the bytes of each unit of `swap` bytes are reversed,
/// as `copySwapMemory0` copies between byte orders. Overlapping ranges of one array are copied
/// as if through a buffer.
pub fn copy_memory<S: RawBytes, D: RawBytes, A: Elements<S>, B: Elements<D>>(
    src: &A,
    src_offset: i64,
    dest: &B,
    dest_offset: i64,
    bytes: i64,
    swap: i64,
) {
    let mut buffer = read_bytes(src, src_offset, bytes);
    swap_units(&mut buffer, swap);
    write_bytes(dest, dest_offset, &buffer);
}

/// `Unsafe.copyMemory0` and `copySwapMemory0` from memory outside the heap, at `address`, into an
/// array of primitives (see `copy_memory`).
pub fn copy_from_native<D: RawBytes, B: Elements<D>>(
    address: i64,
    dest: &B,
    dest_offset: i64,
    bytes: i64,
    swap: i64,
) {
    let mut buffer = crate::read_native(address, native_size(bytes));
    swap_units(&mut buffer, swap);
    write_bytes(dest, dest_offset, &buffer);
}

/// `Unsafe.copyMemory0` and `copySwapMemory0` from an array of primitives into memory outside the
/// heap, at `address` (see `copy_memory`).
pub fn copy_to_native<S: RawBytes, A: Elements<S>>(
    src: &A,
    src_offset: i64,
    address: i64,
    bytes: i64,
    swap: i64,
) {
    let mut buffer = read_bytes(src, src_offset, bytes);
    swap_units(&mut buffer, swap);
    crate::write_native(address, &buffer);
}

/// `Unsafe.copyMemory0` and `copySwapMemory0` within memory outside the heap, from address `src`
/// to address `dest` (overlapping ranges as if through a buffer).
pub fn copy_native(src: i64, dest: i64, bytes: i64, swap: i64) {
    let mut buffer = crate::read_native(src, native_size(bytes));
    swap_units(&mut buffer, swap);
    crate::write_native(dest, &buffer);
}

/// `Unsafe.setMemory0(base, offset, bytes, value)` on an array of primitives: `bytes` bytes of
/// `value` from `offset` (counted from the start of the array object).
pub fn set_memory<D: RawBytes, B: Elements<D>>(array: &B, offset: i64, bytes: i64, value: i8) {
    write_bytes(array, offset, &vec![value as u8; native_size(bytes)]);
}

fn native_size(bytes: i64) -> usize {
    usize::try_from(bytes).unwrap_or_else(|_| unsupported_memory("an Unsafe copy of a negative size"))
}

/// Reverses the bytes of each unit of `swap` bytes, when `swap` is above 1.
fn swap_units(buffer: &mut [u8], swap: i64) {
    if swap > 1 {
        for unit in buffer.chunks_mut(swap as usize) {
            unit.reverse();
        }
    }
}

/// `Unsafe.getInt(base, offset)` and the other plain reads of a primitive at an offset of an array
/// of primitives of any kind (see `read_bytes`): the value its bytes there make.
pub fn get_memory<T: RawBytes, S: RawBytes, A: Elements<S>>(array: &A, offset: i64) -> T {
    T::get(&read_bytes(array, offset, T::SIZE as i64))
}

/// `Unsafe.putInt(base, offset, x)` and the other plain writes of a primitive at an offset of an
/// array of primitives of any kind (see `write_bytes`).
pub fn put_memory<T: RawBytes, D: RawBytes, B: Elements<D>>(array: &B, offset: i64, value: T) {
    let mut bytes = vec![0; T::SIZE];
    value.put(&mut bytes);
    write_bytes(array, offset, &bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::from_vec;
    use std::rc::Rc;

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

    #[test]
    fn copies_memory_between_arrays_of_any_kinds() {
        let base = crate::array_base_offset();
        let bytes = crate::from_vec(vec![1i8, 0, 2, 0, 3, 0]);
        let chars = crate::from_vec(vec![0u16; 3]);
        copy_memory(&bytes, base, &chars, base, 6, 0);
        assert_eq!(chars.take_items(), vec![1, 2, 3]);
        // Within one array, overlapping: as if through a buffer.
        let ints = crate::from_vec(vec![1i32, 2, 3, 4]);
        copy_memory(&ints, base, &ints, base + 4, 12, 0);
        assert_eq!(ints.take_items(), vec![1, 1, 2, 3]);
        // Swapping the bytes of each `char`.
        let swapped = crate::from_vec(vec![0u16; 1]);
        copy_memory(&crate::from_vec(vec![0x0102u16]), base, &swapped, base, 2, 2);
        assert_eq!(swapped.take_items(), vec![0x0201]);
        // Part of a `long`.
        let longs = crate::from_vec(vec![0i64]);
        copy_memory(&crate::from_vec(vec![-1i8; 2]), base, &longs, base + 2, 2, 0);
        assert_eq!(longs.take_items(), vec![0xffff_0000]);
        // A char read from the bytes of a `byte[]`, and an int written over two chars.
        assert_eq!(get_memory::<u16, _, _>(&crate::from_vec(vec![0i8, 0x41, 0x42]), base + 1), 0x4241);
        let two = crate::from_vec(vec![0u16; 2]);
        put_memory(&two, base, 0x0003_0002i32);
        assert_eq!(two.take_items(), vec![2, 3]);
    }
}
