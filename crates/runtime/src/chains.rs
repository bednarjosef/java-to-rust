//! Dropping long chains of objects without deep recursion. Rust drops an object's fields when
//! the object dies, so a linked list of a million nodes would drop its `next` fields a million
//! frames deep and overflow the stack. A class whose objects can lead to objects of the same
//! class gets a `Drop` that moves those fields out of the dying object onto a `Chain`, a stack on
//! the heap, and frees the objects on it one at a time: an object taken from the chain is
//! unlinked the same way before it is freed, so each one dies with its links already gone and
//! the depth stays constant (see docs/object-model.md, "Dropping long chains").
//!
//! An object is put on the chain only while the chain holds its last reference. Any other
//! reference keeps it alive, and dropping the chain's would only decrement the count.
//!
//! Most structures are shallow (a balanced tree of a million nodes is 20 deep), and Rust's own
//! recursive drop is the fastest way to free them. So a `Drop` leaves its fields to Rust while
//! the drops above it use less than `RECURSION_BYTES` of stack, measured from the shallowest
//! `drop_links` this thread has run; only deeper than that does it start a chain.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// How much stack the nested drops of a structure may use, recursively, before a `Drop` moves
/// its links onto a chain. A level of a list or a tree takes a few hundred bytes.
const RECURSION_BYTES: usize = 128 * 1024;

thread_local! {
    /// The highest stack address that a `drop_links` on this thread has run at: the stack
    /// grows down, so the stack that the drops below it use is this minus their address.
    static TOP: Cell<usize> = const { Cell::new(0) };
}

/// An object whose fields may lead to a long chain of objects: a translated class's struct (or
/// its `RefCell`, or a `Vec` of an array).
pub trait Unlink<'a> {
    /// Moves out every field that may lead further down a chain, onto `chain`. Called only on
    /// an object that is dying: in its `Drop`, or when the chain holds its last reference.
    fn unlink(&mut self, chain: &mut Chain<'a>);
}

/// A field's value that may lead to objects of a chain: a handle, an `Option` of one, an array,
/// or the enum of a class type.
pub trait Link<'a> {
    /// Moves what this field of a dying object leads to onto `chain`: a value that can be
    /// `None` is taken out, a handle that cannot is unlinked in place when nothing else holds it.
    fn unlink_in(&mut self, chain: &mut Chain<'a>);

    /// Moves the objects of a value taken out of a field onto `chain`.
    fn into_chain(self, chain: &mut Chain<'a>);
}

/// The objects waiting to be freed, each one held by its last reference.
pub struct Chain<'a> {
    objects: Vec<Rc<dyn Unlink<'a> + 'a>>,
}

impl<'a> Chain<'a> {
    /// `object`, freed later when this is its last reference, and dropped here otherwise (which
    /// only decrements its count).
    #[inline]
    pub fn push<T: Unlink<'a> + 'a>(&mut self, object: Rc<T>) {
        if Rc::strong_count(&object) == 1 && Rc::weak_count(&object) == 0 {
            self.objects.push(object);
        }
    }

    /// Frees the objects on the chain, unlinking each one first, so that freeing it drops no
    /// further object of the chain.
    fn release(&mut self) {
        while let Some(mut object) = self.objects.pop() {
            if let Some(o) = Rc::get_mut(&mut object) {
                o.unlink(self);
            }
        }
    }
}

/// The `Drop` of an object whose fields may lead to a long chain. Near the top of the nested
/// drops it leaves the fields to Rust, which frees them recursively; deeper, it moves them out
/// and frees what they lead to one object at a time.
#[inline(always)]
pub fn drop_links<'a, T: Unlink<'a>>(object: &mut T) {
    // The address of a local: where on the stack this drop runs.
    let marker = 0u8;
    let here = &marker as *const u8 as usize;
    let top = TOP.get();
    if here >= top {
        TOP.set(here);
        return;
    }
    if top - here >= RECURSION_BYTES {
        unlink_deep(object);
    }
}

/// `drop_links` deep in the stack: moves the links onto a chain and frees it. Kept out of line,
/// so that the test above is all that the drop of a shallow object adds.
#[cold]
#[inline(never)]
fn unlink_deep<'a, T: Unlink<'a>>(object: &mut T) {
    let mut chain = Chain { objects: Vec::new() };
    object.unlink(&mut chain);
    if !chain.objects.is_empty() {
        chain.release();
    }
}

impl<'a, T: Unlink<'a> + ?Sized> Unlink<'a> for RefCell<T> {
    #[inline]
    fn unlink(&mut self, chain: &mut Chain<'a>) {
        self.get_mut().unlink(chain);
    }
}

/// The elements of an array.
impl<'a, T: Link<'a>> Unlink<'a> for Vec<T> {
    fn unlink(&mut self, chain: &mut Chain<'a>) {
        for element in self.drain(..) {
            element.into_chain(chain);
        }
    }
}

impl<'a, T: Unlink<'a> + 'a> Link<'a> for Rc<T> {
    #[inline]
    fn unlink_in(&mut self, chain: &mut Chain<'a>) {
        if let Some(o) = Rc::get_mut(self) {
            o.unlink(chain);
        }
    }

    #[inline]
    fn into_chain(self, chain: &mut Chain<'a>) {
        chain.push(self);
    }
}

impl<'a, T: Link<'a>> Link<'a> for Option<T> {
    #[inline]
    fn unlink_in(&mut self, chain: &mut Chain<'a>) {
        if let Some(value) = self.take() {
            value.into_chain(chain);
        }
    }

    #[inline]
    fn into_chain(self, chain: &mut Chain<'a>) {
        if let Some(value) = self {
            value.into_chain(chain);
        }
    }
}

/// An array held as a plain `Vec`.
impl<'a, T: Link<'a>> Link<'a> for Vec<T> {
    fn unlink_in(&mut self, chain: &mut Chain<'a>) {
        Unlink::unlink(self, chain);
    }

    fn into_chain(mut self, chain: &mut Chain<'a>) {
        Unlink::unlink(&mut self, chain);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Node {
        value: i64,
        next: Option<Rc<RefCell<Node>>>,
    }

    impl<'a> Unlink<'a> for Node {
        fn unlink(&mut self, chain: &mut Chain<'a>) {
            Link::unlink_in(&mut self.next, chain);
        }
    }

    impl Drop for Node {
        fn drop(&mut self) {
            drop_links(self);
        }
    }

    struct Tree<T> {
        children: crate::Array<Option<Rc<Tree<T>>>>,
        _value: T,
    }

    impl<'a, T: 'a> Unlink<'a> for Tree<T> {
        fn unlink(&mut self, chain: &mut Chain<'a>) {
            Link::unlink_in(&mut self.children, chain);
        }
    }

    impl<T> Drop for Tree<T> {
        fn drop(&mut self) {
            drop_links(self);
        }
    }

    /// Runs `f` on a thread with a small stack, where a recursive drop of the chains below
    /// would overflow.
    fn on_small_stack(f: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new().stack_size(512 * 1024).spawn(f).unwrap().join().unwrap();
    }

    #[test]
    fn a_long_list_drops() {
        on_small_stack(|| {
            let mut head = None;
            for value in 0..1_000_000 {
                head = Some(Rc::new(RefCell::new(Node { value, next: head })));
            }
            let kept = head.as_ref().map(|h| h.borrow().next.clone()).unwrap();
            drop(head);
            // The rest of the list is still alive through `kept`.
            assert_eq!(kept.as_ref().map(|n| n.borrow().value), Some(999_998));
        });
    }

    #[test]
    fn a_deep_tree_of_arrays_drops() {
        on_small_stack(|| {
            let mut tree = Rc::new(Tree { children: crate::from_vec(vec![]), _value: String::new() });
            for _ in 0..200_000 {
                tree = Rc::new(Tree {
                    children: crate::from_vec(vec![Some(tree), None]),
                    _value: String::new(),
                });
            }
        });
    }
}
