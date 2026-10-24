//! Java's `StringBuilder`. A builder is a mutable object that every holder of a reference sees
//! change, so it is a shared handle, like `java::Array`: cloning it gives another name for the
//! same builder. Its text is UTF-16 code units, as in Java, so `charAt`, `setCharAt`, `insert`
//! and `reverse` work on units exactly, and a lone surrogate stays one, in its text (`text`) too;
//! it prints as `?`, as Java prints it.
//!
//! Each method borrows the cell only while it runs, so its arguments may read the builder
//! (`sb.append(sb.length())`), and the methods return the builder for chaining
//! (`sb.append(a).append(b)`).
//!
//! The cell is an `Rc<RefCell<..>>` in a program without threads (`StringBuilder`), and an
//! `Arc<java::Locked<..>>` in one with threads (`SharedStringBuilder`), whose builders several
//! threads may reach, as its arrays are `java::SharedArray`s. Both have the same methods.

use crate::Arc;
use crate::Locked;
use crate::string::from_units;
use crate::throw;
use crate::{JString, Wtf8};
use std::cell::RefCell;
use std::fmt::{self, Write};
use std::ops::{Deref, DerefMut};

/// The shared cell of a builder's code units.
pub trait BuilderCell: Clone {
    type Units<'a>: Deref<Target = Vec<u16>>
    where
        Self: 'a;
    type UnitsMut<'a>: DerefMut<Target = Vec<u16>>
    where
        Self: 'a;
    fn new(units: Vec<u16>) -> Self;
    fn units(&self) -> Self::Units<'_>;
    fn units_mut(&self) -> Self::UnitsMut<'_>;
    /// The cell's address, the builder's identity.
    fn address(&self) -> *const u8;
}

impl BuilderCell for Arc<RefCell<Vec<u16>>> {
    type Units<'a> = std::cell::Ref<'a, Vec<u16>>;
    type UnitsMut<'a> = std::cell::RefMut<'a, Vec<u16>>;
    fn new(units: Vec<u16>) -> Self {
        Arc::new(RefCell::new(units))
    }
    fn units(&self) -> Self::Units<'_> {
        self.borrow()
    }
    fn units_mut(&self) -> Self::UnitsMut<'_> {
        self.borrow_mut()
    }
    fn address(&self) -> *const u8 {
        Arc::as_ptr(self) as *const u8
    }
}

impl BuilderCell for Arc<Locked<Vec<u16>>> {
    type Units<'a> = crate::Ref<'a, Vec<u16>>;
    type UnitsMut<'a> = crate::RefMut<'a, Vec<u16>>;
    fn new(units: Vec<u16>) -> Self {
        Arc::new(Locked::new(units))
    }
    fn units(&self) -> Self::Units<'_> {
        self.borrow()
    }
    fn units_mut(&self) -> Self::UnitsMut<'_> {
        self.borrow_mut()
    }
    fn address(&self) -> *const u8 {
        Arc::as_ptr(self) as *const u8
    }
}

/// A builder whose code units are in the cell `C`.
#[derive(Clone)]
#[repr(transparent)]
pub struct Builder<C>(C);

// A builder is its handle.
unsafe impl<C: crate::Pointer> crate::Pointer for Builder<C> {}

/// The builder of a program without threads.
pub type StringBuilder = Builder<Arc<RefCell<Vec<u16>>>>;

/// The builder of a program with threads.
pub type SharedStringBuilder = Builder<Arc<Locked<Vec<u16>>>>;

impl<C: BuilderCell> Default for Builder<C> {
    fn default() -> Self {
        Builder(C::new(Vec::new()))
    }
}

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

impl<C: BuilderCell> Builder<C> {
    /// `new StringBuilder()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `new StringBuilder(capacity)`.
    pub fn with_capacity(capacity: i32) -> Self {
        if capacity < 0 {
            throw("java.lang.NegativeArraySizeException", &capacity.to_string());
        }
        Builder(C::new(Vec::with_capacity(capacity as usize)))
    }

    /// `new StringBuilder(s)`.
    pub fn from_text(s: &Wtf8) -> Self {
        let b = Self::new();
        b.append_str(s);
        b
    }

    /// `a == b`: whether two handles name the same builder.
    pub fn same(&self, other: &Self) -> bool {
        self.0.address() == other.0.address()
    }

    /// `sb.append(s)` of a string.
    pub fn append_str(&self, s: &Wtf8) -> &Self {
        let mut value = self.0.units_mut();
        if s.is_ascii() {
            value.extend(s.as_bytes().iter().map(|&b| u16::from(b)));
        } else {
            value.extend(s.units());
        }
        drop(value);
        self
    }

    /// `sb.append(x)` of a value whose `Display` is Java's text for it, such as an `int` or a
    /// `java::Double`. It must not read this builder, which is borrowed meanwhile.
    pub fn append<T: fmt::Display>(&self, value: T) -> &Self {
        let _ = write!(Units(&mut self.0.units_mut()), "{value}");
        self
    }

    /// `sb.append(c)` of a `char`.
    pub fn append_char(&self, c: u16) -> &Self {
        self.0.units_mut().push(c);
        self
    }

    /// `sb.append(other)` of a builder, which may be this one.
    pub fn append_builder(&self, other: &Self) -> &Self {
        let copy = other.0.units().clone();
        self.0.units_mut().extend_from_slice(&copy);
        self
    }

    /// `sb.append(chars)` of a `char[]`.
    pub fn append_chars(&self, chars: &[u16]) -> &Self {
        self.0.units_mut().extend_from_slice(chars);
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

    /// `sb.append(s, start, end)` of a part of a string. The check is Java 26's
    /// `Preconditions.checkFromToIndex` on `s.length()`, so a null `s` (already "null") is checked
    /// as its four characters.
    pub fn append_range(&self, s: &Wtf8, start: i32, end: i32) -> &Self {
        self.try_append_range(s, start, end).unwrap_or_else(crate::uncaught)
    }

    /// `append_range` where the program observes the `IndexOutOfBoundsException`.
    pub fn try_append_range(&self, s: &Wtf8, start: i32, end: i32) -> Result<&Self, crate::Exception> {
        let units: Vec<u16> = s.units().collect();
        if start < 0 || start > end || end as usize > units.len() {
            let message = format!("Range [{start}, {end}) out of bounds for length {}", units.len());
            return Err(crate::Exception::new("java.lang.IndexOutOfBoundsException", message));
        }
        Ok(self.append_chars(&units[start as usize..end as usize]))
    }

    /// `sb.append(s, start, end)` of a `CharSequence` that a program class implements, where
    /// `length` and `char_at` are its methods. `length` runs once and the range is checked on it;
    /// then `char_at` runs for each index in order. The units are taken before the builder
    /// changes, so a failure leaves the builder as it was, as Java's `append` does.
    pub fn try_append_seq_range<E: From<crate::Exception>>(
        &self,
        start: i32,
        end: i32,
        length: Result<i32, E>,
        mut char_at: impl FnMut(i32) -> Result<u16, E>,
    ) -> Result<&Self, E> {
        let len = length?;
        if start < 0 || start > end || end > len {
            let message = format!("Range [{start}, {end}) out of bounds for length {len}");
            return Err(crate::Exception::new("java.lang.IndexOutOfBoundsException", message).into());
        }
        let units = (start..end).map(&mut char_at).collect::<Result<Vec<u16>, E>>()?;
        Ok(self.append_chars(&units))
    }

    /// `append_seq_range` where the program does not observe the failure.
    pub fn append_seq_range<E: From<crate::Exception> + std::fmt::Display + 'static>(
        &self,
        start: i32,
        end: i32,
        length: Result<i32, E>,
        char_at: impl FnMut(i32) -> Result<u16, E>,
    ) -> &Self {
        self.try_append_seq_range(start, end, length, char_at).unwrap_or_else(crate::uncaught)
    }

    fn check_offset(&self, offset: i32, len: usize) {
        if offset < 0 || offset as usize > len {
            out_of_bounds(format!("offset {offset}, length {len}"));
        }
    }

    fn insert_units(&self, offset: i32, units: &[u16]) -> &Self {
        let mut value = self.0.units_mut();
        self.check_offset(offset, value.len());
        let at = offset as usize;
        value.splice(at..at, units.iter().copied());
        self
    }

    /// `sb.insert(offset, s)` of a string.
    pub fn insert_str(&self, offset: i32, s: &Wtf8) -> &Self {
        let units: Vec<u16> = s.units().collect();
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
    pub fn insert_builder(&self, offset: i32, other: &Self) -> &Self {
        let copy = other.0.units().clone();
        self.insert_units(offset, &copy)
    }

    /// `sb.reverse()`: the code units in reverse order, except that each surrogate pair stays a
    /// pair.
    pub fn reverse(&self) -> &Self {
        let mut value = self.0.units_mut();
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
            out_of_bounds(format!("Index {index} out of bounds for length {len}"));
        }
        index as usize
    }

    /// `sb.charAt(index)`.
    pub fn char_at(&self, index: i32) -> u16 {
        let value = self.0.units();
        value[Self::check_index(index, value.len())]
    }

    /// `sb.setCharAt(index, c)`.
    pub fn set_char_at(&self, index: i32, c: u16) {
        let mut value = self.0.units_mut();
        let i = Self::check_index(index, value.len());
        value[i] = c;
    }

    /// `sb.deleteCharAt(index)`.
    pub fn delete_char_at(&self, index: i32) -> &Self {
        let mut value = self.0.units_mut();
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
        let mut value = self.0.units_mut();
        let (start, end) = Self::range(start, end, value.len());
        value.drain(start..end);
        drop(value);
        self
    }

    /// `sb.replace(start, end, s)`.
    pub fn replace(&self, start: i32, end: i32, s: &Wtf8) -> &Self {
        let mut value = self.0.units_mut();
        let (start, end) = Self::range(start, end, value.len());
        value.splice(start..end, s.units());
        drop(value);
        self
    }

    /// `sb.toString()`: the text, exactly.
    pub fn text(&self) -> JString {
        from_units(&self.0.units())
    }

    /// `sb.length()`.
    pub fn length(&self) -> i32 {
        self.0.units().len() as i32
    }

    /// `sb.isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.0.units().is_empty()
    }

    /// `sb.setLength(len)`: cut, or padded with `'\0'`.
    pub fn set_length(&self, len: i32) {
        if len < 0 {
            out_of_bounds(format!("String index out of range: {len}"));
        }
        self.0.units_mut().resize(len as usize, 0);
    }

    /// `sb.ensureCapacity(minimumCapacity)`: room for that many characters; a capacity that is
    /// not positive asks for nothing.
    pub fn ensure_capacity(&self, minimum: i32) {
        let mut value = self.0.units_mut();
        let more = usize::try_from(minimum).unwrap_or(0).saturating_sub(value.len());
        value.reserve(more);
    }

    /// `sb.indexOf(s, from)`.
    pub fn index_of(&self, s: &Wtf8, from: i32) -> i32 {
        let value = self.0.units();
        let target: Vec<u16> = s.units().collect();
        let from = from.clamp(0, value.len() as i32) as usize;
        if target.len() > value.len() - from {
            return -1;
        }
        (from..=value.len() - target.len())
            .find(|&i| value[i..].starts_with(&target))
            .map_or(-1, |i| i as i32)
    }

    /// `sb.lastIndexOf(s, from)`.
    pub fn last_index_of(&self, s: &Wtf8, from: i32) -> i32 {
        let value = self.0.units();
        let target: Vec<u16> = s.units().collect();
        let from = (from as i64).min(value.len() as i64 - target.len() as i64);
        if from < 0 {
            return -1;
        }
        (0..=from as usize).rev().find(|&i| value[i..].starts_with(&target)).map_or(-1, |i| i as i32)
    }

    /// `sb.substring(start, end)`.
    pub fn substring(&self, start: i32, end: i32) -> JString {
        let value = self.0.units();
        if start < 0 || start > end || end as usize > value.len() {
            out_of_bounds(format!("Range [{start}, {end}) out of bounds for length {}", value.len()));
        }
        from_units(&value[start as usize..end as usize])
    }

    /// `sb.substring(start)`.
    pub fn substring_from(&self, start: i32) -> JString {
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
            self.0.units_mut().extend(units.iter().cycle().take(units.len() * count as usize));
        }
        self
    }

    /// `sb.repeat(s, count)`: `count` copies of a string.
    pub fn repeat_str(&self, s: &Wtf8, count: i32) -> &Self {
        if check_count(count) {
            let units: Vec<u16> = s.units().collect();
            self.0.units_mut().extend(units.iter().cycle().take(units.len() * count as usize));
        }
        self
    }

    /// `sb.repeat(other, count)` of a builder, which may be this one.
    pub fn repeat_builder(&self, other: &Self, count: i32) -> &Self {
        if check_count(count) {
            let copy = other.0.units().clone();
            self.0.units_mut().extend(copy.iter().cycle().take(copy.len() * count as usize));
        }
        self
    }

    /// `sb.codePointAt(index)`.
    pub fn code_point_at(&self, index: i32) -> i32 {
        crate::string::code_point_at_units(&self.0.units(), index)
    }

    /// `sb.codePointBefore(index)`.
    pub fn code_point_before(&self, index: i32) -> i32 {
        crate::string::code_point_before_units(&self.0.units(), index)
    }

    /// `sb.codePointCount(begin, end)`.
    pub fn code_point_count(&self, begin: i32, end: i32) -> i32 {
        crate::string::code_point_count_units(&self.0.units(), begin, end)
    }

    /// `sb.offsetByCodePoints(index, offset)`.
    pub fn offset_by_code_points(&self, index: i32, offset: i32) -> i32 {
        crate::string::offset_by_code_points_units(&self.0.units(), index, offset)
    }

    /// `sb.getChars(begin, end, dst, dst_begin)`: the units `begin..end` into `dst` from
    /// `dst_begin`.
    pub fn get_chars(&self, begin: i32, end: i32, dst: &mut [u16], dst_begin: i32) {
        let value = self.0.units();
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

    /// `sb.getChars(begin, end, dst, dst_begin)` on a builder held as a `CharSequence`, whose
    /// `dst` may be null: Java checks the range on the builder before it reads `dst.length`.
    pub fn sequence_get_chars(
        &self,
        begin: i32,
        end: i32,
        dst: Option<&impl crate::Elements<u16>>,
        dst_begin: i32,
    ) {
        match dst {
            Some(dst) => self.get_chars(begin, end, &mut dst.items_mut(), dst_begin),
            None => {
                self.substring(begin, end);
                crate::throw_exception(crate::string::null_destination())
            }
        }
    }

    /// `a.compareTo(b)`: the difference of the first code units that differ, or of the lengths.
    pub fn compare_to(&self, other: &Self) -> i32 {
        let (a, b) = (self.0.units().clone(), other.0.units().clone());
        match a.iter().zip(&b).find(|(x, y)| x != y) {
            Some((x, y)) => *x as i32 - *y as i32,
            None => a.len() as i32 - b.len() as i32,
        }
    }
}

/// `a == b` on builders that may be null.
pub fn same_builder<C: BuilderCell>(a: Option<&Builder<C>>, b: Option<&Builder<C>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.same(b),
        (a, b) => a.is_none() && b.is_none(),
    }
}

/// `sb.toString()`, and how a builder prints.
/// A builder's identity is its cell's address, which `identity_hash` hashes.
impl<C: BuilderCell> crate::Handle for Builder<C> {
    fn address(&self) -> *const u8 {
        self.0.address()
    }
}

/// A builder's monitor is its cell's: the lock of a shared builder, which is also the lock its
/// methods take, as an object's is (see monitor.rs).
impl<C: BuilderCell + crate::HasMonitor> crate::HasMonitor for Builder<C> {
    #[inline]
    fn monitor(&self) -> crate::MonitorOf<'_> {
        self.0.monitor()
    }
}

/// A builder in a string concatenation: its text as it is, lone surrogates and all.
impl<C: BuilderCell> crate::Piece for Builder<C> {
    fn push_to(&self, out: &mut JString) {
        for &unit in self.0.units().iter() {
            out.push_unit(unit);
        }
    }
}

impl<C: BuilderCell> fmt::Display for Builder<C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for c in char::decode_utf16(self.0.units().iter().copied()) {
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
        sb.append_str(Wtf8::new("hello")).append(42).append_char(b'!' as u16);
        assert_eq!(sb.to_string(), "hello42!");
        sb.insert_str(0, Wtf8::new(">> ")).delete_char_at(1).reverse();
        assert_eq!(sb.to_string(), "!24olleh >");
        sb.set_length(3);
        sb.replace(1, 100, Wtf8::new("xyz"));
        assert_eq!(sb.to_string(), "!xyz");
        assert_eq!(sb.index_of(Wtf8::new("yz"), 0), 2);
        assert_eq!(sb.length(), 4);
    }

    #[test]
    fn reverse_keeps_surrogate_pairs() {
        let sb = StringBuilder::from_text(Wtf8::new("a😀b"));
        sb.reverse();
        assert_eq!(sb.to_string(), "b😀a");
    }

    #[test]
    fn a_shared_builder_is_seen_changed_by_another_thread() {
        crate::locked::begin_threads();
        let sb = SharedStringBuilder::from_text(Wtf8::new("ab"));
        let other = sb.clone();
        std::thread::spawn(move || {
            other.append_str(Wtf8::new("cd"));
        })
        .join()
        .unwrap();
        sb.append_builder(&sb.clone());
        assert_eq!(sb.to_string(), "abcdabcd");
        assert!(same_builder(Some(&sb), Some(&sb.clone())));
    }

    #[test]
    fn appending_itself_copies_first() {
        let sb = StringBuilder::from_text(Wtf8::new("ab"));
        sb.append_builder(&sb.clone());
        assert_eq!(sb.to_string(), "abab");
    }
}
