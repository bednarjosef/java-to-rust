//! Java's `StringBuilder`. A builder is a mutable object that every holder of a reference sees
//! change, so it is a shared handle, like `java::Array`: cloning it gives another name for the
//! same builder. Its text is UTF-16 code units, as in Java, so `charAt`, `setCharAt`, `insert`
//! and `reverse` work on units exactly, and a lone surrogate stays one until the text is
//! converted to a Rust string, where it becomes `?`, as Java prints it.
//!
//! Each method borrows the cell only while it runs, so its arguments may read the builder
//! (`sb.append(sb.length())`), and the methods return the builder for chaining
//! (`sb.append(a).append(b)`).

use crate::string::from_units;
use crate::throw;
use std::cell::RefCell;
use std::fmt::{self, Write};
use std::rc::Rc;

#[derive(Clone, Default)]
pub struct StringBuilder(Rc<RefCell<Vec<u16>>>);

/// The UTF-16 units of a code point, which must be valid, as `Character.toChars` makes them.
fn code_point_units(cp: i32) -> Vec<u16> {
    if !(0..=0x10FFFF).contains(&cp) {
        throw("java.lang.IllegalArgumentException", &format!("Not a valid Unicode code point: 0x{cp:X}"));
    }
    if cp < 0x10000 {
        return vec![cp as u16];
    }
    let v = cp - 0x10000;
    vec![0xD800 + (v >> 10) as u16, 0xDC00 + (v & 0x3FF) as u16]
}

/// Whether `repeat` adds anything: a negative count throws, and zero adds nothing.
fn check_count(count: i32) -> bool {
    if count < 0 {
        throw("java.lang.IllegalArgumentException", &format!("count is negative: {count}"));
    }
    count > 0
}

/// Writes formatted text into a builder's code units.
struct Units<'a>(&'a mut Vec<u16>);

impl Write for Units<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if s.is_ascii() {
            self.0.extend(s.bytes().map(u16::from));
        } else {
            self.0.extend(s.encode_utf16());
        }
        Ok(())
    }
}

fn out_of_bounds(message: String) -> ! {
    throw("java.lang.StringIndexOutOfBoundsException", &message)
}

impl StringBuilder {
    /// `new StringBuilder()`.
    pub fn new() -> StringBuilder {
        StringBuilder::default()
    }

    /// `new StringBuilder(capacity)`.
    pub fn with_capacity(capacity: i32) -> StringBuilder {
        if capacity < 0 {
            throw("java.lang.NegativeArraySizeException", &capacity.to_string());
        }
        StringBuilder(Rc::new(RefCell::new(Vec::with_capacity(capacity as usize))))
    }

    /// `new StringBuilder(s)`.
    pub fn from_text(s: &str) -> StringBuilder {
        let b = StringBuilder::new();
        b.append_str(s);
        b
    }

    /// `a == b`: whether two handles name the same builder.
    pub fn same(&self, other: &StringBuilder) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// `sb.append(s)` of a string.
    pub fn append_str(&self, s: &str) -> &Self {
        let _ = Units(&mut self.0.borrow_mut()).write_str(s);
        self
    }

    /// `sb.append(x)` of a value whose `Display` is Java's text for it, such as an `int` or a
    /// `java::Double`. It must not read this builder, which is borrowed meanwhile.
    pub fn append<T: fmt::Display>(&self, value: T) -> &Self {
        let _ = write!(Units(&mut self.0.borrow_mut()), "{value}");
        self
    }

    /// `sb.append(c)` of a `char`.
    pub fn append_char(&self, c: u16) -> &Self {
        self.0.borrow_mut().push(c);
        self
    }

    /// `sb.append(other)` of a builder, which may be this one.
    pub fn append_builder(&self, other: &StringBuilder) -> &Self {
        let copy = other.0.borrow().clone();
        self.0.borrow_mut().extend_from_slice(&copy);
        self
    }

    /// `sb.append(chars)` of a `char[]`.
    pub fn append_chars(&self, chars: &[u16]) -> &Self {
        self.0.borrow_mut().extend_from_slice(chars);
        self
    }

    /// `sb.append(chars, offset, len)`.
    pub fn append_chars_range(&self, chars: &[u16], offset: i32, len: i32) -> &Self {
        let end = offset as i64 + len as i64;
        if offset < 0 || len < 0 || end > chars.len() as i64 {
            throw(
                "java.lang.IndexOutOfBoundsException",
                &format!("Range [{offset}, {offset} + {len}) out of bounds for length {}", chars.len()),
            );
        }
        self.append_chars(&chars[offset as usize..end as usize])
    }

    /// `sb.append(s, start, end)` of a part of a string.
    pub fn append_range(&self, s: &str, start: i32, end: i32) -> &Self {
        let units: Vec<u16> = s.encode_utf16().collect();
        if start < 0 || start > end || end as usize > units.len() {
            throw(
                "java.lang.IndexOutOfBoundsException",
                &format!("start {start}, end {end}, length {}", units.len()),
            );
        }
        self.append_chars(&units[start as usize..end as usize])
    }

    fn check_offset(&self, offset: i32, len: usize) {
        if offset < 0 || offset as usize > len {
            out_of_bounds(format!("offset {offset}, length {len}"));
        }
    }

    fn insert_units(&self, offset: i32, units: &[u16]) -> &Self {
        let mut value = self.0.borrow_mut();
        self.check_offset(offset, value.len());
        let at = offset as usize;
        value.splice(at..at, units.iter().copied());
        self
    }

    /// `sb.insert(offset, s)` of a string.
    pub fn insert_str(&self, offset: i32, s: &str) -> &Self {
        let units: Vec<u16> = s.encode_utf16().collect();
        self.insert_units(offset, &units)
    }

    /// `sb.insert(offset, x)` of a value shown through its `Display`.
    pub fn insert<T: fmt::Display>(&self, offset: i32, value: T) -> &Self {
        let mut units = Vec::new();
        let _ = write!(Units(&mut units), "{value}");
        self.insert_units(offset, &units)
    }

    /// `sb.insert(offset, c)` of a `char`.
    pub fn insert_char(&self, offset: i32, c: u16) -> &Self {
        self.insert_units(offset, &[c])
    }

    /// `sb.insert(offset, chars)` of a `char[]`.
    pub fn insert_chars(&self, offset: i32, chars: &[u16]) -> &Self {
        self.insert_units(offset, chars)
    }

    /// `sb.insert(offset, other)` of a builder, which may be this one.
    pub fn insert_builder(&self, offset: i32, other: &StringBuilder) -> &Self {
        let copy = other.0.borrow().clone();
        self.insert_units(offset, &copy)
    }

    /// `sb.reverse()`: the code units in reverse order, except that each surrogate pair stays a
    /// pair.
    pub fn reverse(&self) -> &Self {
        let mut value = self.0.borrow_mut();
        value.reverse();
        let mut i = 0;
        while i + 1 < value.len() {
            if (0xDC00..0xE000).contains(&value[i]) && (0xD800..0xDC00).contains(&value[i + 1]) {
                value.swap(i, i + 1);
                i += 1;
            }
            i += 1;
        }
        self
    }

    fn check_index(index: i32, len: usize) -> usize {
        if index < 0 || index as usize >= len {
            out_of_bounds(format!("index {index},length {len}"));
        }
        index as usize
    }

    /// `sb.charAt(index)`.
    pub fn char_at(&self, index: i32) -> u16 {
        let value = self.0.borrow();
        value[Self::check_index(index, value.len())]
    }

    /// `sb.setCharAt(index, c)`.
    pub fn set_char_at(&self, index: i32, c: u16) {
        let mut value = self.0.borrow_mut();
        let i = Self::check_index(index, value.len());
        value[i] = c;
    }

    /// `sb.deleteCharAt(index)`.
    pub fn delete_char_at(&self, index: i32) -> &Self {
        let mut value = self.0.borrow_mut();
        let i = Self::check_index(index, value.len());
        value.remove(i);
        drop(value);
        self
    }

    /// The range `[start, end)` of `delete` and `replace`: `end` is cut to the length.
    fn range(start: i32, end: i32, len: usize) -> (usize, usize) {
        let end = end.min(len as i32);
        if start < 0 || start > end {
            out_of_bounds(format!("begin {start}, end {end}, length {len}"));
        }
        (start as usize, end as usize)
    }

    /// `sb.delete(start, end)`.
    pub fn delete(&self, start: i32, end: i32) -> &Self {
        let mut value = self.0.borrow_mut();
        let (start, end) = Self::range(start, end, value.len());
        value.drain(start..end);
        drop(value);
        self
    }

    /// `sb.replace(start, end, s)`.
    pub fn replace(&self, start: i32, end: i32, s: &str) -> &Self {
        let mut value = self.0.borrow_mut();
        let (start, end) = Self::range(start, end, value.len());
        value.splice(start..end, s.encode_utf16());
        drop(value);
        self
    }

    /// `sb.length()`.
    pub fn length(&self) -> i32 {
        self.0.borrow().len() as i32
    }

    /// `sb.isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.0.borrow().is_empty()
    }

    /// `sb.setLength(len)`: cut, or padded with `'\0'`.
    pub fn set_length(&self, len: i32) {
        if len < 0 {
            out_of_bounds(format!("String index out of range: {len}"));
        }
        self.0.borrow_mut().resize(len as usize, 0);
    }

    /// `sb.indexOf(s, from)`.
    pub fn index_of(&self, s: &str, from: i32) -> i32 {
        let value = self.0.borrow();
        let target: Vec<u16> = s.encode_utf16().collect();
        let from = from.clamp(0, value.len() as i32) as usize;
        if target.len() > value.len() - from {
            return -1;
        }
        (from..=value.len() - target.len())
            .find(|&i| value[i..].starts_with(&target))
            .map_or(-1, |i| i as i32)
    }

    /// `sb.lastIndexOf(s, from)`.
    pub fn last_index_of(&self, s: &str, from: i32) -> i32 {
        let value = self.0.borrow();
        let target: Vec<u16> = s.encode_utf16().collect();
        let from = (from as i64).min(value.len() as i64 - target.len() as i64);
        if from < 0 {
            return -1;
        }
        (0..=from as usize).rev().find(|&i| value[i..].starts_with(&target)).map_or(-1, |i| i as i32)
    }

    /// `sb.substring(start, end)`.
    pub fn substring(&self, start: i32, end: i32) -> String {
        let value = self.0.borrow();
        if start < 0 || start > end || end as usize > value.len() {
            out_of_bounds(format!("start {start}, end {end}, length {}", value.len()));
        }
        from_units(&value[start as usize..end as usize])
    }

    /// `sb.substring(start)`.
    pub fn substring_from(&self, start: i32) -> String {
        let len = self.length();
        self.substring(start, len)
    }

    /// `sb.appendCodePoint(cp)`: one unit, or a surrogate pair for a supplementary code point.
    pub fn append_code_point(&self, cp: i32) -> &Self {
        let units = code_point_units(cp);
        self.append_chars(&units)
    }

    /// `sb.repeat(cp, count)`: `count` copies of a code point.
    pub fn repeat_code_point(&self, cp: i32, count: i32) -> &Self {
        if check_count(count) {
            let units = code_point_units(cp);
            self.0.borrow_mut().extend(units.iter().cycle().take(units.len() * count as usize));
        }
        self
    }

    /// `sb.repeat(s, count)`: `count` copies of a string.
    pub fn repeat_str(&self, s: &str, count: i32) -> &Self {
        if check_count(count) {
            let units: Vec<u16> = s.encode_utf16().collect();
            self.0.borrow_mut().extend(units.iter().cycle().take(units.len() * count as usize));
        }
        self
    }

    /// `sb.repeat(other, count)` of a builder, which may be this one.
    pub fn repeat_builder(&self, other: &StringBuilder, count: i32) -> &Self {
        if check_count(count) {
            let copy = other.0.borrow().clone();
            self.0.borrow_mut().extend(copy.iter().cycle().take(copy.len() * count as usize));
        }
        self
    }

    /// `sb.codePointAt(index)`.
    pub fn code_point_at(&self, index: i32) -> i32 {
        crate::string::code_point_at_units(&self.0.borrow(), index)
    }

    /// `sb.codePointBefore(index)`.
    pub fn code_point_before(&self, index: i32) -> i32 {
        crate::string::code_point_before_units(&self.0.borrow(), index)
    }

    /// `sb.codePointCount(begin, end)`.
    pub fn code_point_count(&self, begin: i32, end: i32) -> i32 {
        crate::string::code_point_count_units(&self.0.borrow(), begin, end)
    }

    /// `sb.offsetByCodePoints(index, offset)`.
    pub fn offset_by_code_points(&self, index: i32, offset: i32) -> i32 {
        crate::string::offset_by_code_points_units(&self.0.borrow(), index, offset)
    }

    /// `sb.getChars(begin, end, dst, dst_begin)`: the units `begin..end` into `dst` from
    /// `dst_begin`.
    pub fn get_chars(&self, begin: i32, end: i32, dst: &mut [u16], dst_begin: i32) {
        let value = self.0.borrow();
        if begin < 0 || begin > end || end as usize > value.len() {
            out_of_bounds(format!("Range [{begin}, {end}) out of bounds for length {}", value.len()));
        }
        let count = (end - begin) as usize;
        if dst_begin < 0 || dst_begin as i64 + count as i64 > dst.len() as i64 {
            throw(
                "java.lang.IndexOutOfBoundsException",
                &format!(
                    "Range [{dst_begin}, {}) out of bounds for length {}",
                    dst_begin as i64 + count as i64,
                    dst.len()
                ),
            );
        }
        let at = dst_begin as usize;
        dst[at..at + count].copy_from_slice(&value[begin as usize..end as usize]);
    }

    /// `a.compareTo(b)`: the difference of the first code units that differ, or of the lengths.
    pub fn compare_to(&self, other: &StringBuilder) -> i32 {
        let (a, b) = (self.0.borrow().clone(), other.0.borrow().clone());
        match a.iter().zip(&b).find(|(x, y)| x != y) {
            Some((x, y)) => *x as i32 - *y as i32,
            None => a.len() as i32 - b.len() as i32,
        }
    }
}

/// `a == b` on builders that may be null.
pub fn same_builder(a: Option<&StringBuilder>, b: Option<&StringBuilder>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.same(b),
        (a, b) => a.is_none() && b.is_none(),
    }
}

/// `sb.toString()`, and how a builder prints.
/// A builder's identity is its cell's address, which `identity_hash` hashes.
impl crate::Handle for StringBuilder {
    fn address(&self) -> *const u8 {
        Rc::as_ptr(&self.0) as *const u8
    }
}

impl fmt::Display for StringBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for c in char::decode_utf16(self.0.borrow().iter().copied()) {
            f.write_char(c.unwrap_or('?'))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builders_edit_code_units() {
        let sb = StringBuilder::new();
        sb.append_str("hello").append(42).append_char(b'!' as u16);
        assert_eq!(sb.to_string(), "hello42!");
        sb.insert_str(0, ">> ").delete_char_at(1).reverse();
        assert_eq!(sb.to_string(), "!24olleh >");
        sb.set_length(3);
        sb.replace(1, 100, "xyz");
        assert_eq!(sb.to_string(), "!xyz");
        assert_eq!(sb.index_of("yz", 0), 2);
        assert_eq!(sb.length(), 4);
    }

    #[test]
    fn reverse_keeps_surrogate_pairs() {
        let sb = StringBuilder::from_text("a😀b");
        sb.reverse();
        assert_eq!(sb.to_string(), "b😀a");
    }

    #[test]
    fn appending_itself_copies_first() {
        let sb = StringBuilder::from_text("ab");
        sb.append_builder(&sb.clone());
        assert_eq!(sb.to_string(), "abab");
    }
}
