//! `System.arraycopy` between arrays of references whose element classes need a check, when one
//! of the two arrays is plain (a `Vec`). Two arrays behind handles are copied by
//! `natives::try_arraycopy_with`.

use crate::natives::copy_bounds;
use crate::{Exception, Source, Target};

/// `arraycopy` from an array of references into one of other references, which the checks of
/// HotSpot decide: `convert` gives each element as the destination's, or the message of the
/// `ArrayStoreException` for it. The elements before the failing one are stored and the rest are
/// not.
pub fn try_copy_refs<T: Clone, U: Clone>(
    src: &(impl Source<T> + ?Sized),
    src_pos: i32,
    dest: &mut (impl Target<U> + ?Sized),
    dest_pos: i32,
    length: i32,
    convert: impl Fn(&T) -> Result<U, &'static str>,
) -> Result<(), Exception> {
    let (from, to, n) = copy_bounds::<T>(src.length(), src_pos, dest.length(), dest_pos, length)
        .map_err(|message| Exception::new("java.lang.ArrayIndexOutOfBoundsException", message))?;
    let (stored, failure) = converted(&src.range(from..from + n), &convert);
    dest.store(to, &stored);
    match failure {
        Some(message) => Err(Exception::new("java.lang.ArrayStoreException", message)),
        None => Ok(()),
    }
}

/// The elements `convert` gives for `items`, up to the first it refuses, and the message of the
/// `ArrayStoreException` for that one.
pub(crate) fn converted<T, U>(
    items: &[T],
    convert: &impl Fn(&T) -> Result<U, &'static str>,
) -> (Vec<U>, Option<&'static str>) {
    let mut stored = Vec::with_capacity(items.len());
    for item in items {
        match convert(item) {
            Ok(u) => stored.push(u),
            Err(message) => return (stored, Some(message)),
        }
    }
    (stored, None)
}
