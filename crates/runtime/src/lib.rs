//! Java semantics that Rust does not have built in, for code produced by `java-to-rust`:
//! printing doubles the way Java does, integer division that throws `ArithmeticException`,
//! Java's `Math` rules and uncaught exceptions.

mod array;
mod compact;
mod slice;

pub use array::*;
pub use slice::*;

use std::fmt;

mod allocation_counts;
mod arguments;
mod boxed;
mod builder;
mod catching;
mod ceil_div;
mod character;
mod character_classes;
mod character_tables;
mod charset;
mod class;
mod code_point;
mod code_point_case_tables;
mod code_point_chars;
mod code_point_tables;
mod copy_refs;
mod deflate;
mod dispatch_counts;
mod exact;
mod exception;
mod fd;
pub mod fdlibm;
mod field;
mod file_channel;
mod file_channels;
mod file_copy;
mod fixed;
mod float;
mod float_parse;
pub mod format;
mod fs;
pub mod gc;
mod hex_float;
mod hot_text;
mod inflate;
mod initializer;
mod integer;
mod integer_parse;
mod java_arrays;
mod lent;
mod locked;
mod math;
mod memory_access;
mod monitor;
mod native_memory;
mod natives;
mod net;
mod null;
mod out;
mod parse_range;
mod preconditions;
mod process;
mod proxy_classes;
mod random;
pub mod regex;
mod returned;
mod scalb;
mod shared;
mod shared_string;
mod signals;
#[cfg(unix)]
mod socket;
mod statics;
pub mod string;
mod system;
mod thin;
mod thread;
mod time_zone;
mod translate_escapes;
mod unix;
mod unsigned;
mod vm;
mod weak;
pub mod wtf8;
pub use allocation_counts::{AllocationCount, count_allocation};
pub use arguments::main_args;
pub use boxed::*;
pub use builder::{Builder, BuilderCell, SharedStringBuilder, StringBuilder, same_builder};
pub use catching::{ExceptionsHeld, catching, exceptions_held, unwinding};
pub use ceil_div::*;
pub use character::*;
pub use character_classes::*;
pub use charset::*;
pub use class::{Class, ClassKind, ClassSet, Nest};
pub use code_point::{
    char_count, code_point_string, high_surrogate, is_bmp_code_point, is_supplementary_code_point,
    is_valid_code_point, low_surrogate, to_chars, to_code_point,
};
pub use code_point_chars::*;
pub use compact::{Compress, RefField, SharedRefField};
pub use copy_refs::*;
pub use deflate::{
    deflate_adler, deflate_buffer_buffer, deflate_buffer_bytes, deflate_bytes, deflate_bytes_buffer,
    deflate_end, deflate_init, deflate_reset, deflate_set_dictionary, deflate_set_dictionary_buffer,
};
pub use dispatch_counts::{DispatchCount, count_dispatch};
pub use exact::*;
pub use exception::*;
pub use fd::*;
pub use field::{Field, SharedField};
pub use file_channel::*;
pub use file_channels::*;
pub use file_copy::*;
pub use fixed::{FixedFields, FixedHandle};
pub use float::*;
pub use float_parse::{parse_double, parse_float, try_parse_double, try_parse_float};
pub use format::{Arg, format, format_objects, print_format, print_format_objects};
pub use fs::*;
pub use gc::references::{has_pending_references, wait_for_pending_references};
pub use gc::{Allocator, CompactAllocator, compact_references};
pub use hex_float::{double_to_hex_string, float_to_hex_string};
pub use inflate::{
    inflate_adler, inflate_buffer_buffer, inflate_buffer_bytes, inflate_bytes, inflate_bytes_buffer,
    inflate_end, inflate_init, inflate_reset, inflate_set_dictionary, inflate_set_dictionary_buffer,
};
pub use initializer::*;
pub use integer::*;
pub use integer_parse::{
    decode, decode_small, parse_radix, parse_small_radix, try_decode, try_decode_small, try_parse_radix,
    try_parse_small_radix,
};
pub use java_arrays::*;
pub use lent::{Lent, LentMut, Source, Target, copy_range};
pub use locked::{Locked, RawLock, Ref, RefMut};
pub use math::*;
pub use memory_access::memory_access_warning;
pub use monitor::*;
pub use native_memory::*;
pub use natives::*;
pub use net::*;
pub use null::{
    Handle, OrNull, SameObject, assigned, assigned_ref, non_null, non_null_message, or_null, same,
    same_object, try_non_null, try_non_null_message, unkept_identity,
};
pub use out::{
    Out, drain_stdout, eprint_chars, eprintln_chars, err, err_flush, flush, out, print_chars, println_chars,
    standard_close, standard_flush, standard_write, standard_write_bytes, write_byte,
};
pub use parse_range::{parse_range, parse_unsigned_range, try_parse_range, try_parse_unsigned_range};
pub use preconditions::*;
pub use process::{destroy_process, spawn_process, wait_for_process_exit};
pub use proxy_classes::{annotations_made, in_dynamic_module, name_proxy_class, note_annotation};
pub use random::*;
pub use regex::{matches_all, replace_all, replace_first, split};
pub use returned::{Ret, Widened};
pub use scalb::*;
pub use shared::{Arc, Slot};
pub use shared_string::*;
#[cfg(unix)]
pub use socket::*;
pub use statics::*;
pub use string::*;
pub use system::*;
pub use thin::{Pointer, Thin, bad_tag};
pub use thread::*;
pub use time_zone::*;
pub use translate_escapes::*;
pub use unix::*;
pub use unsigned::{
    compare_unsigned, divide_unsigned, parse_unsigned, remainder_unsigned, to_unsigned_string,
    try_parse_unsigned,
};
pub use vm::*;
pub use weak::{WeakField, register_reference, take_pending_reference};
pub use wtf8::{JString, Piece, Wtf8, string_case, wtf8, wtf8_bytes};

/// Formats a `double` like Java's `Double.toString`: `1.0`, `0.001`, `1.0E7`, `4.9E-324`.
pub struct Double(pub f64);

impl fmt::Display for Double {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&double_to_string(self.0))
    }
}

pub fn double_to_string(d: f64) -> String {
    if d.is_nan() {
        return "NaN".into();
    }
    let sign = if d.is_sign_negative() { "-" } else { "" };
    if d.is_infinite() {
        return format!("{sign}Infinity");
    }
    if d == 0.0 {
        return format!("{sign}0.0");
    }
    layout(sign, shortest_digits(d.abs()))
}

/// Lays out the digits and exponent of a positive number the way `Double.toString` and
/// `Float.toString` do: plain from `0.001` up to `10^7`, scientific (`1.0E7`) outside that.
fn layout(sign: &str, (digits, exp): (String, i32)) -> String {
    if (-3..7).contains(&exp) {
        let (int, frac) = if exp >= 0 {
            let int_len = exp as usize + 1;
            if digits.len() <= int_len {
                (format!("{digits:0<int_len$}"), "0".to_string())
            } else {
                (digits[..int_len].to_string(), digits[int_len..].to_string())
            }
        } else {
            ("0".to_string(), format!("{}{digits}", "0".repeat((-exp - 1) as usize)))
        };
        format!("{sign}{int}.{frac}")
    } else {
        let rest = if digits.len() > 1 { &digits[1..] } else { "0" };
        format!("{sign}{}.{rest}E{exp}", &digits[..1])
    }
}

/// The decimal digits and exponent Java picks for a positive finite `d`: the shortest decimal
/// that rounds to `d`, except that when one digit would do, Java takes the closest decimal of
/// at most two digits (so `Double.MIN_VALUE` prints as `4.9E-324`, not `5.0E-324`). Among
/// decimals of that length, Java takes the closest to `d`, and of two equally close, the one
/// with an even last digit (so `1100000.25f` prints as `1100000.2`).
pub(crate) fn shortest_digits<T: fmt::LowerExp + std::str::FromStr + PartialEq>(d: T) -> (String, i32) {
    let split = |s: String| {
        let (mantissa, exp) = s.split_once('e').expect("{:e} output has an exponent");
        (mantissa.replace('.', ""), exp.parse::<i32>().expect("exponent is an integer"))
    };
    let (digits, exp) = split(format!("{d:e}"));
    // Exact formatting rounds to the closest decimal of the given length, ties to even. It is
    // Java's choice whenever it still rounds to `d`; the shortest form is not always it.
    let len = digits.len().max(2);
    let closest = format!("{d:.*e}", len - 1);
    if closest.parse::<T>().ok() == Some(d) {
        let (closest_digits, closest_exp) = split(closest);
        return (closest_digits.trim_end_matches('0').to_string(), closest_exp);
    }
    (digits, exp)
}

/// Integer types with Java's division rules.
pub trait Integer: Copy {
    fn java_div(self, rhs: Self) -> Self;
    fn java_rem(self, rhs: Self) -> Self;
    fn try_div(self, rhs: Self) -> Result<Self, Exception>;
    fn try_rem(self, rhs: Self) -> Result<Self, Exception>;
}

macro_rules! integer {
    ($($t:ty),*) => {$(
        impl Integer for $t {
            fn java_div(self, rhs: Self) -> Self {
                if rhs == 0 {
                    throw("java.lang.ArithmeticException", "/ by zero");
                }
                self.wrapping_div(rhs)
            }

            fn java_rem(self, rhs: Self) -> Self {
                if rhs == 0 {
                    throw("java.lang.ArithmeticException", "/ by zero");
                }
                self.wrapping_rem(rhs)
            }

            fn try_div(self, rhs: Self) -> Result<Self, Exception> {
                if rhs == 0 {
                    return Err(Exception::new("java.lang.ArithmeticException", "/ by zero"));
                }
                Ok(self.wrapping_div(rhs))
            }

            fn try_rem(self, rhs: Self) -> Result<Self, Exception> {
                if rhs == 0 {
                    return Err(Exception::new("java.lang.ArithmeticException", "/ by zero"));
                }
                Ok(self.wrapping_rem(rhs))
            }
        }
    )*};
}

integer!(i32, i64);

/// Java's `(int)`, `(long)` of a `double` or `float`: truncates toward zero, saturates at the
/// type's bounds, and gives 0 for NaN, as Rust's `as` does. Rust's `as` clamps the value with a
/// `max` and a `min` before it converts and tests for NaN after; on x86-64 the conversion
/// instruction alone gives the exact result for every value in range, and the type's `MIN` for
/// the others, so only that result is converted again, with `as`.
macro_rules! float_to_integer {
    ($($name:ident: $from:ty => $to:ty, $set:ident, $convert:ident;)*) => {$(
        #[inline(always)]
        pub fn $name(v: $from) -> $to {
            #[cfg(target_arch = "x86_64")]
            {
                use std::arch::x86_64::{$convert, $set};
                // SSE2 is part of x86-64, so the intrinsics are safe to call.
                #[allow(unused_unsafe)]
                let converted = unsafe { $convert($set(v)) };
                if converted != <$to>::MIN {
                    return converted;
                }
                #[cold]
                #[inline(never)]
                fn saturate(v: $from) -> $to {
                    v as $to
                }
                saturate(v)
            }
            #[cfg(not(target_arch = "x86_64"))]
            {
                v as $to
            }
        }
    )*};
}

float_to_integer! {
    double_to_int: f64 => i32, _mm_set_sd, _mm_cvttsd_si32;
    double_to_long: f64 => i64, _mm_set_sd, _mm_cvttsd_si64;
    float_to_int: f32 => i32, _mm_set_ss, _mm_cvttss_si32;
    float_to_long: f32 => i64, _mm_set_ss, _mm_cvttss_si64;
}

/// Java's `/` on `int` and `long`: truncates, wraps `MIN / -1`, throws on zero.
pub fn div<T: Integer>(a: T, b: T) -> T {
    a.java_div(b)
}

/// Java's `%` on `int` and `long`.
pub fn rem<T: Integer>(a: T, b: T) -> T {
    a.java_rem(b)
}

/// `/` where the program observes the `ArithmeticException` of a division by zero.
pub fn try_div<T: Integer>(a: T, b: T) -> Result<T, Exception> {
    a.try_div(b)
}

/// `%` where the program observes the `ArithmeticException` of a division by zero.
pub fn try_rem<T: Integer>(a: T, b: T) -> Result<T, Exception> {
    a.try_rem(b)
}

/// Java's `Math.max(double, double)`: NaN wins, and `0.0` is larger than `-0.0`.
pub fn max(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if a == 0.0 && b == 0.0 && a.is_sign_negative() {
        b
    } else if a >= b {
        a
    } else {
        b
    }
}

/// Java's `Math.min(double, double)`: NaN wins, and `-0.0` is smaller than `0.0`.
pub fn min(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if a == 0.0 && b == 0.0 && b.is_sign_negative() {
        b
    } else if a <= b {
        a
    } else {
        b
    }
}

/// Java's `String.length()`, which counts UTF-16 code units.
#[inline]
pub fn string_length<S: Text + ?Sized>(s: &S) -> i32 {
    if s.ascii_text() { s.wtf8_text().len() as i32 } else { s.wtf8_text().unit_len() as i32 }
}

/// Java's `System.exit`.
pub fn exit(code: i32) -> ! {
    thread::run_shutdown_hooks();
    // What `System.out` still buffers is lost, as Java loses it.
    drain_stdout();
    std::process::exit(code)
}

/// Ends a trial translation (`--census` with an output) where it reaches code that the
/// translator rejected: `message` names the rejection and where it is. The exit status is 70
/// (`EX_SOFTWARE`), which no Java program's uncaught exception gives.
pub fn not_translated(message: &str) -> ! {
    drain_stdout();
    eprintln!("not translated: {message}");
    std::process::exit(70)
}

/// Ends the program the way an uncaught Java exception of the standard class `class` does.
pub fn throw(class: &str, message: &str) -> ! {
    catching::raise(class, message);
    // The class's name lives on for an uncaught-exception handler, while the program ends.
    let class: &'static str = Box::leak(class.to_string().into_boxed_str());
    exception::die(Exception::new(class, message))
}

/// `throw` of an exception made already, such as one without a message.
pub fn throw_exception(e: Exception) -> ! {
    exception::die(catching::raise_exception(e))
}

/// What a field updater (`AtomicIntegerFieldUpdater` and its kin) throws for a null object: its
/// access check finds that the object is not of the field's class, a `ClassCastException`
/// without a message.
pub fn updater_refused() -> ! {
    throw_exception(Exception::bare("java.lang.ClassCastException"))
}

/// A class as a `ClassCastException` names it: its binary name, and the module and class loader
/// that HotSpot names with it ("unnamed module of loader 'app'").
pub type CastClass = (&'static str, &'static str);

/// The module of a class of the JDK's `java.base`, as HotSpot names it with the class.
pub const BASE: &str = "module java.base of loader 'bootstrap'";

/// The unnamed module of the system class loader, where a program's classes are.
pub const APP: &str = "unnamed module of loader 'app'";

/// What a `ClassCastException` names of the object that a cast fails on: the class of the
/// object. The translation implements it for each enum of classes that it casts from.
pub trait CastSource {
    fn cast_class(&self) -> CastClass;
}

impl<T: CastSource + ?Sized> CastSource for &T {
    fn cast_class(&self) -> CastClass {
        (**self).cast_class()
    }
}

/// HotSpot's message for a cast of an object of class `from` to class `to` that fails:
/// "class A cannot be cast to class B (A and B are in unnamed module of loader 'app')", or with
/// each class's module of its own when they differ.
pub fn cast_message(from: CastClass, to: CastClass) -> String {
    // A proxy class's name and module are those it was given at run time (`proxy_classes`).
    let shown = |(name, module): CastClass| proxy_classes::proxy_class(name).unwrap_or((name, module));
    let ((a, a_module), (b, b_module)) = (shown(from), shown(to));
    let modules = if a_module == b_module {
        format!("{a} and {b} are in {a_module}")
    } else {
        format!("{a} is in {a_module}; {b} is in {b_module}")
    };
    format!("class {a} cannot be cast to class {b} ({modules})")
}

/// Java's checked cast `(T) x` on a non-null object `from`: the object as a `T` if it is one,
/// which the translation finds (`|o| o.as_t()`), else a `ClassCastException`. `to` is a
/// reference to a constant, so that where the cast is inlined its failure is one call with two
/// pointers (`cast_failed_on`), cheap enough not to keep the caller from being inlined in turn.
#[inline]
pub fn cast<S: CastSource + ?Sized, T>(
    from: &S,
    as_t: impl FnOnce(&S) -> Option<T>,
    to: &'static CastClass,
) -> T {
    match as_t(from) {
        Some(value) => value,
        None => cast_failed_on(from, to),
    }
}

/// The value of a test of a class that the translation proved passes (`|o| o.as_t()` on an object
/// whose class an earlier test on the same value already found). The test stays, with its failure
/// out of line (`proven_wrong`): where the earlier test is in the same function after inlining,
/// the optimizer folds the two, and a proof that is wrong stops the program rather than letting
/// it run on with a value of the wrong class.
#[inline(always)]
pub fn proven<T>(value: Option<T>) -> T {
    match value {
        Some(value) => value,
        None => proven_wrong(),
    }
}

/// The end of the program where a class test that the translation proved passes failed.
#[cold]
#[inline(never)]
pub fn proven_wrong() -> ! {
    panic!("a class test that the translation proved passes failed")
}

/// `cast`, where a `catch` or a `finally` sees its `ClassCastException`: the failure is
/// returned, to go on as a thrown exception does.
#[inline]
pub fn try_cast<S: CastSource + ?Sized, T>(
    from: &S,
    as_t: impl FnOnce(&S) -> Option<T>,
    to: &'static CastClass,
) -> Result<T, Exception> {
    match as_t(from) {
        Some(value) => Ok(value),
        None => Err(cast_exception_on(from, to)),
    }
}

/// The `ClassCastException` of a cast of an object of class `from` to class `to`.
#[cold]
pub fn cast_exception(from: CastClass, to: CastClass) -> Exception {
    Exception::new("java.lang.ClassCastException", cast_message(from, to))
}

/// Throws the `ClassCastException` of a cast of an object of class `from` to class `to`.
#[cold]
pub fn cast_failed(from: CastClass, to: CastClass) -> ! {
    throw("java.lang.ClassCastException", &cast_message(from, to))
}

/// `cast_failed` of the object `from`, whose class it asks out of the line of the cast.
#[cold]
#[inline(never)]
pub fn cast_failed_on<S: CastSource + ?Sized>(from: &S, to: &'static CastClass) -> ! {
    cast_failed(from.cast_class(), *to)
}

/// `cast_exception` of the object `from`, out of the line of the cast (`cast_failed_on`).
#[cold]
#[inline(never)]
pub fn cast_exception_on<S: CastSource + ?Sized>(from: &S, to: &'static CastClass) -> Exception {
    cast_exception(from.cast_class(), *to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cast_messages_are_hotspots() {
        assert_eq!(
            cast_message(("Main$Dog", APP), ("Main$Cat", APP)),
            "class Main$Dog cannot be cast to class Main$Cat (Main$Dog and Main$Cat are in unnamed \
             module of loader 'app')"
        );
        assert_eq!(
            cast_message(("java.lang.Integer", BASE), ("Main$Cat", APP)),
            "class java.lang.Integer cannot be cast to class Main$Cat (java.lang.Integer is in module \
             java.base of loader 'bootstrap'; Main$Cat is in unnamed module of loader 'app')"
        );
    }

    #[test]
    fn ties_between_shortest_decimals_take_the_even_digit() {
        // 2^49 + 0.25 is exactly between ...312.2 and ...312.3 at 16 digits.
        assert_eq!(double_to_string(f64::from_bits(0x4300_0000_0000_0002)), "5.629499534213122E14");
    }

    #[test]
    fn doubles_print_like_java() {
        let cases = [
            (1.0, "1.0"),
            (100.0, "100.0"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1.0 / 3.0, "0.3333333333333333"),
            (0.001, "0.001"),
            (0.0001, "1.0E-4"),
            (1234567.0, "1234567.0"),
            (1e7, "1.0E7"),
            (12345678.9, "1.23456789E7"),
            (-0.0, "-0.0"),
            (f64::MIN_POSITIVE, "2.2250738585072014E-308"),
            (f64::from_bits(1), "4.9E-324"),
            (f64::MAX, "1.7976931348623157E308"),
            (2e23, "2.0E23"),
            (f64::NAN, "NaN"),
            (f64::NEG_INFINITY, "-Infinity"),
        ];
        for (value, java) in cases {
            assert_eq!(double_to_string(value), java, "{value:e}");
        }
    }
}
