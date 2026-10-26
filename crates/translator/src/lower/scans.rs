//! The scans of a program's trees whose answers are the same in each attempt of lowering
//! (`real_generics::retry`): which standard classes and functional interfaces the code uses,
//! what of `Object` it needs, the names its variables have, and, in each member, the local and
//! anonymous classes, the lambdas and what its body asks about its caller. An attempt hands its
//! answers on to the next, which takes one only when it asks of the very same trees (and, for
//! the interfaces, the same classes declared); the trees are borrowed for all of the attempts,
//! so the same pointers are the same trees and the answer is the one a scan would give.

use super::Node;
use super::callers::CallerQuery;
use super::exceptions::Standard;
use super::functional::Functional;
use crate::value::Value;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// The answers so far, shared by the attempts of one lowering.
#[derive(Default)]
pub(super) struct Scans<'t> {
    /// The standard classes used, with the trees they were found in.
    standard: RefCell<Option<(Trees, Rc<Standard>)>>,
    /// The functional interfaces used, with the trees and the classes declared.
    functional: RefCell<Option<(FunctionalKey, Rc<Functional>)>>,
    /// The uses of `Object`, with the trees.
    object: RefCell<Option<(Trees, ObjectUses)>>,
    /// The names that the variables of each class's code have (`fresh_name`), by the class's
    /// tree.
    declared_names: RefCell<HashMap<*const Value, Rc<HashSet<String>>>>,
    /// The local and anonymous classes in the code of each member (`declare_local_classes`).
    local_classes: RefCell<HashMap<*const Value, LocalClasses<'t>>>,
    /// The lambdas and method references in each member, in the order of the source, each with
    /// the one it is in (`declare_lambdas`).
    lambdas: RefCell<HashMap<*const Value, Lambdas<'t>>>,
    /// What each method's body asks about its caller (`asks_caller`), by the body.
    caller_queries: RefCell<HashMap<*const Value, Option<CallerQuery>>>,
}

/// The trees that a scan was made of, which are the same trees when the pointers are the same.
type Trees = Vec<*const Value>;

/// The trees that functional interfaces were looked for in, and the classes declared then.
type FunctionalKey = (Trees, HashSet<String>);

/// The local and anonymous classes in a member's code, each with its `new` when it is anonymous.
pub(super) type LocalClasses<'t> = Rc<Vec<(Node<'t>, Option<Node<'t>>)>>;

/// The lambdas in a member, each with the one it is in.
pub(super) type Lambdas<'t> = Rc<Vec<(&'t Value, Option<&'t Value>)>>;

/// What the code in the trees that `plan_object` plans for uses of `Object`, as far as that
/// depends on the trees alone.
#[derive(Clone, Copy)]
pub(super) struct ObjectUses {
    pub comparable: bool,
    pub number: bool,
    pub char_sequence: bool,
    pub record: bool,
    pub enumeration: bool,
    pub array_text: bool,
}

impl<'t> Scans<'t> {
    /// `Standard::select` of these trees.
    pub fn standard(&self, trees: &[&Value]) -> Rc<Standard> {
        let key: Trees = trees.iter().map(|&t| t as *const Value).collect();
        let mut cached = self.standard.borrow_mut();
        match &*cached {
            Some((k, standard)) if *k == key => standard.clone(),
            _ => {
                let standard = Rc::new(Standard::select(trees.iter().copied()));
                *cached = Some((key, standard.clone()));
                standard
            }
        }
    }

    /// `Functional::select` of these trees, for these declared classes.
    pub fn functional(&self, trees: &[&Value], declared: &HashSet<String>) -> Rc<Functional> {
        let key: Trees = trees.iter().map(|&t| t as *const Value).collect();
        let mut cached = self.functional.borrow_mut();
        match &*cached {
            Some(((k, d), functional)) if *k == key && d == declared => functional.clone(),
            _ => {
                let functional = Rc::new(Functional::select(trees.iter().copied(), declared));
                *cached = Some(((key, declared.clone()), functional.clone()));
                functional
            }
        }
    }

    /// The uses of `Object` in these trees, which `find` finds.
    pub fn object(&self, trees: &[&Value], find: impl FnOnce() -> ObjectUses) -> ObjectUses {
        let key: Trees = trees.iter().map(|&t| t as *const Value).collect();
        let mut cached = self.object.borrow_mut();
        match &*cached {
            Some((k, uses)) if *k == key => *uses,
            _ => {
                let uses = find();
                *cached = Some((key, uses));
                uses
            }
        }
    }

    /// The names of the variables of the code in the class `tree`, which `find` finds.
    pub fn declared_names(
        &self,
        tree: &Value,
        find: impl FnOnce() -> HashSet<String>,
    ) -> Rc<HashSet<String>> {
        memo(&self.declared_names, tree, || Rc::new(find()))
    }

    /// The local and anonymous classes in the code of `member`, which `find` finds.
    pub fn local_classes(
        &self,
        member: Node<'t>,
        find: impl FnOnce() -> Vec<(Node<'t>, Option<Node<'t>>)>,
    ) -> LocalClasses<'t> {
        memo(&self.local_classes, member.0, || Rc::new(find()))
    }

    /// The lambdas in `member`, which `find` finds.
    pub fn lambdas(
        &self,
        member: &'t Value,
        find: impl FnOnce() -> Vec<(&'t Value, Option<&'t Value>)>,
    ) -> Lambdas<'t> {
        memo(&self.lambdas, member, || Rc::new(find()))
    }

    /// What the method body `body` asks about its caller, which `find` finds.
    pub fn caller_query(
        &self,
        body: &Value,
        find: impl FnOnce() -> Option<CallerQuery>,
    ) -> Option<CallerQuery> {
        memo(&self.caller_queries, body, find)
    }
}

/// The answer kept in `answers` for `tree`, or the one that `find` finds, kept for next time.
fn memo<T: Clone>(answers: &RefCell<HashMap<*const Value, T>>, tree: &Value, find: impl FnOnce() -> T) -> T {
    let key = tree as *const Value;
    if let Some(answer) = answers.borrow().get(&key) {
        return answer.clone();
    }
    let answer = find();
    answers.borrow_mut().insert(key, answer.clone());
    answer
}
