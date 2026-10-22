//! `System.arraycopy` between arrays of references whose element classes need a check, when one
//! of the two arrays is plain (a `Vec`, which has no shared borrow to take). Two shared arrays are
//! copied by `natives::try_arraycopy_with`.

use crate::Exception;
use crate::natives::copy_bounds;

/// `arraycopy` from a slice of references into one of other references, which the checks of
/// HotSpot decide: `convert` gives each element as the destination's, or the message of the
/// `ArrayStoreException` for it. The elements before the failing one are stored and the rest are
/// not.
pub fn try_copy_refs<T: Clone, U: Clone>(
    src: &[T],
    src_pos: i32,
    dest: &mut [U],
    dest_pos: i32,
    length: i32,
    convert: impl Fn(&T) -> Result<U, &'static str>,
) -> Result<(), Exception> {
    let (from, to, n) = copy_bounds::<T>(src.len(), src_pos, dest.len(), dest_pos, length)
        .map_err(|message| Exception::new("java.lang.ArrayIndexOutOfBoundsException", message))?;
    let mut stored = Vec::with_capacity(n);
    let mut failure = None;
    for item in &src[from..from + n] {
        match convert(item) {
            Ok(u) => stored.push(u),
            Err(message) => {
                failure = Some(message);
                break;
            }
        }
    }
    dest[to..to + stored.len()].clone_from_slice(&stored);
    match failure {
        Some(message) => Err(Exception::new("java.lang.ArrayStoreException", message)),
        None => Ok(()),
    }
}
