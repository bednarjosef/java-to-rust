//! The instance field initializers that raise. Java runs them in each constructor of their class,
//! outside every `try` of the constructor, so what they raise escapes it. The class's `field_inits`
//! then returns a `Result`, and the constructors that run it pass the failure on.

use super::super::FastSet;
use super::*;

/// What a point raises, given the escapes of every method (see `analyze`).
type Raised<'a> = dyn Fn(&Point, &[BTreeSet<Kind>]) -> BTreeSet<Kind> + 'a;

/// The classes (by index) with an instance field initializer whose points raise something that
/// escapes it. `raised` and `escapes` are those of `analyze`.
pub(super) fn throwing(
    class: &Class,
    facts: &Facts,
    raised: &Raised<'_>,
    escapes: &[BTreeSet<Kind>],
) -> FastSet<usize> {
    let mut out = FastSet::default();
    for (t, decl) in class.types.iter().enumerate() {
        let raises = decl.fields.iter().filter_map(|f| f.init.as_ref()).any(|init| {
            let mut points = Vec::new();
            expr_points(class, facts, usize::MAX, init, &[], &[], &mut points);
            points.iter().any(|p| !raised(p, escapes).is_empty())
        });
        if raises {
            out.insert(t);
        }
    }
    out
}
