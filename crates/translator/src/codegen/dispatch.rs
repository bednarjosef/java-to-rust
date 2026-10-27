//! Class trees with several concrete classes. A value of a class type that objects of several
//! concrete classes can be is an enum named after the type, with a variant per concrete class
//! holding that class's object (`Rc<RefCell<Circle>>`, or `Rc<Circle>` in an immutable family):
//!
//! - a call on such a value is a *dispatcher*, a method of the enum that matches the variant and
//!   calls what the object's class runs;
//! - a field is read through `base()`, which borrows the fields struct of the type's class from
//!   whichever object the value is, and written through `base_mut()`;
//! - `==` compares the objects' identity, `PartialEq` on the enum;
//! - printing goes through the enum's `Display`, which shows each object as its class does;
//! - a body inherited from the type that needs the whole object (not only its fields) is a
//!   method of the enum, taking `&self`;
//! - a subtype's value converts to a supertype's with `to_<supertype>()`, and an object of a
//!   concrete class is wrapped in its variant.
//!
//! Each item of an enum is written only when some code uses it. The generator learns which by
//! generating the program until no new item is asked for (see `generate`).

use super::arraycopy::CopyCall;
use super::*;
use crate::analysis::{ENUM_COMPARE_NULL, compare_null_message};
use std::collections::{BTreeMap, BTreeSet};

/// How `same_variant` compares two objects of a variant that is equal only to itself.
const IDENTITY: &str = "Arc::ptr_eq(a, b)";

/// The number of each type in a `java::Thin` (`Gen::class_tag`), by its index in `Class::types`:
/// the class trees in depth-first preorder, each class before its subclasses, then the types that
/// no class tree holds (interfaces). The classes at or below a class then have the numbers from
/// its own to its last subclass's, so a test of "is a `C`" is one range (`Gen::tags`). Numbers
/// start at 1, as a `Thin` is never 0, and are the same in every pass of the generation.
pub(super) fn class_numbers(class: &Class) -> Vec<u16> {
    let h = &class.hierarchy;
    let count = class.types.len();
    let tree = |t: usize| !matches!(class.types[t].kind, TypeKind::Interface | TypeKind::Param);
    let mut order = Vec::with_capacity(count);
    let mut stack = Vec::new();
    for root in (0..count).filter(|&t| tree(t) && h.superclass(t).is_none()) {
        stack.push(root);
        while let Some(t) = stack.pop() {
            order.push(t);
            // The direct subclasses, pushed last first so that they come out in index order.
            stack.extend(h.subtypes[t].iter().rev().copied().filter(|&s| h.superclass(s) == Some(t)));
        }
    }
    let mut numbered = vec![false; count];
    for &t in &order {
        numbered[t] = true;
    }
    order.extend((0..count).filter(|&t| !numbered[t]));
    let mut numbers = vec![0u16; count];
    for (at, &t) in order.iter().enumerate() {
        numbers[t] =
            u16::try_from(at + 1).ok().filter(|&tag| tag != u16::MAX).expect("fewer than 65535 classes");
    }
    numbers
}

/// The most classes a call's receiver may be of for the call to dispatch among them alone
/// (`Need::Among`).
const AMONG_MOST: usize = 4;

/// The key of a set of classes in `Class::receivers`: a hash of it, the same in every pass of the
/// generation and every translation.
pub(super) fn receiver_key(classes: &[usize]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::hash::DefaultHasher::new();
    classes.hash(&mut hasher);
    hasher.finish()
}

/// An item of the enum of a type that the code being generated uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Need {
    /// The enum of the type, with its `Clone` and its identity.
    Enum(usize),
    /// `base()`: the fields struct of the type's class, borrowed from the variant's object.
    Base(usize),
    /// `base_mut()`: the same, borrowed mutably.
    BaseMut(usize),
    /// The dispatcher on an enum for an override group (by group id).
    Dispatch(usize, usize),
    /// The dispatcher on an enum for an override group whose objects are known to be of a few of
    /// its classes, the set of a call's receiver in `Class::receivers` (by its `receiver_key`):
    /// arms for those, and the whole dispatcher for any other.
    Among(usize, usize, u64),
    /// A method whose body is emitted on the enum of its type.
    Body(usize),
    /// `to_<t>()` on the enum of a subtype `s`: the same object as a value of `t`, as `(s, t)`.
    Convert(usize, usize),
    /// The enum's `Display`.
    Display(usize),
    /// `nullable_text()`: what the object's `toString` returns, null as `None`, or its exception.
    NullableText(usize),
    /// `java_string()`: the string that the object's `toString` returns, which does not throw.
    JavaString(usize),
    /// `as_<t>()` on the enum of `s`: the object as a value of type `t` if it is one, as
    /// `(s, t)`.
    Narrow(usize, usize),
    /// `runtime_class()`: the `java::Class` of the object's class (see `class_values`).
    RuntimeClass(usize),
    /// `runtime_class_name()`: the binary name of the object's class.
    RuntimeClassName(usize),
    /// `runtime_type_names()`: the binary names of the object's class and of every class and
    /// interface above it (see `class_values::instance_names`).
    RuntimeTypeNames(usize),
    /// `object_identity_hash()`: `System.identityHashCode` of the object.
    IdentityHash(usize),
    /// `java::CastSource`: the object's class as a `ClassCastException` names it.
    CastSource(usize),
    /// `array_copy_kind()`: what the object is to `System.arraycopy` (`java::CopyKind`).
    CopyKind(usize),
    /// `java::CopyAcross`: what `System.arraycopy` between arrays of two classes asks of the
    /// object (see `arraycopy`).
    CopyAcross(usize),
}

/// The items of `wanted` written in the `impl` of the enum of each type (`type_enum_items`), in
/// their order.
pub(super) fn impl_needs(wanted: &Needs, homes: &[Home]) -> HashMap<usize, Vec<Need>> {
    let mut out: HashMap<usize, Vec<Need>> = HashMap::new();
    for &need in wanted {
        let t = match need {
            Need::Base(s)
            | Need::BaseMut(s)
            | Need::Convert(s, _)
            | Need::Narrow(s, _)
            | Need::Dispatch(s, _)
            | Need::Among(s, _, _) => s,
            Need::Body(i) => match homes[i] {
                Home::Enum(t) => t,
                _ => continue,
            },
            _ => continue,
        };
        out.entry(t).or_default().push(need);
    }
    out
}

/// The types whose enum items read whether `need` is wanted (`Gen::type_enum_items`): the type
/// it is of, and for a method's body, the type on whose enum it is written or, for a closure
/// type, whose trait has it.
pub(super) fn need_types(class: &Class, homes: &[Home], need: Need) -> Vec<usize> {
    match need {
        Need::Body(i) => {
            let mut types = vec![class.methods[i].class];
            if let Home::Enum(t) = homes[i] {
                types.push(t);
            }
            types
        }
        Need::Convert(s, t) | Need::Narrow(s, t) => vec![s, t],
        Need::Enum(t)
        | Need::Base(t)
        | Need::BaseMut(t)
        | Need::Dispatch(t, _)
        | Need::Among(t, _, _)
        | Need::Display(t)
        | Need::NullableText(t)
        | Need::JavaString(t)
        | Need::RuntimeClass(t)
        | Need::RuntimeClassName(t)
        | Need::RuntimeTypeNames(t)
        | Need::IdentityHash(t)
        | Need::CastSource(t)
        | Need::CopyKind(t)
        | Need::CopyAcross(t) => vec![t],
    }
}

/// `text` as the operand of a method call or field access: in parentheses unless it is a path,
/// a call or a field.
pub(super) fn operand(text: &str) -> String {
    let plain =
        text.chars().all(|ch| ch.is_alphanumeric() || "_:.()&".contains(ch)) && !text.starts_with(['&', '*']);
    if plain { text.to_string() } else { format!("({text})") }
}

/// How a value of one class type becomes a value of another, in a cast or an `instanceof`.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Narrowing {
    /// The value's type is the target type or below it: every object of it is one of the target.
    Up,
    /// The value is of the enum of this type, whose variants are tested.
    Down(usize),
    /// The value's class is known, and whether it is of the target type.
    Known(bool),
}

impl<'a> Gen<'a> {
    /// Records that the code uses an item of an enum (and so the enum).
    pub(super) fn need(&self, need: Need) {
        let mut needs = self.needs.borrow_mut();
        match need {
            Need::Base(t)
            | Need::BaseMut(t)
            | Need::Dispatch(t, _)
            | Need::Display(t)
            | Need::RuntimeClass(t)
            | Need::RuntimeClassName(t)
            | Need::RuntimeTypeNames(t)
            | Need::IdentityHash(t)
            | Need::CastSource(t)
            | Need::CopyKind(t) => {
                needs.insert(Need::Enum(t));
            }
            Need::NullableText(t) | Need::JavaString(t) => {
                needs.insert(Need::Enum(t));
                needs.insert(Need::Display(t));
            }
            Need::Convert(s, t) => {
                needs.insert(Need::Enum(s));
                needs.insert(Need::Enum(t));
            }
            Need::CopyAcross(t) => {
                needs.extend([Need::Enum(t), Need::CopyKind(t), Need::RuntimeClass(t)]);
            }
            Need::Narrow(s, _) => {
                needs.insert(Need::Enum(s));
            }
            Need::Among(t, group, _) => {
                needs.insert(Need::Enum(t));
                needs.insert(Need::Dispatch(t, group));
            }
            Need::Body(i) => {
                if let Home::Enum(t) = self.homes[i] {
                    needs.insert(Need::Enum(t));
                }
            }
            Need::Enum(_) => {}
        }
        needs.insert(need);
    }

    /// The type whose enum represents the values of class type `ty`, if an enum does.
    pub(super) fn enum_type(&self, ty: Type) -> Option<usize> {
        self.enum_for(self.class.class_index(ty)?)
    }

    /// The type whose enum represents the values of class `t`, if an enum does.
    pub(super) fn enum_for(&self, t: usize) -> Option<usize> {
        match self.rep(t) {
            hierarchy::Rep::Enum(t) => Some(t),
            hierarchy::Rep::Object(_) => None,
        }
    }

    /// The type whose enum the value of `e` is. `this` is the enum in a method of the enum, and
    /// an object of the class being written anywhere else.
    pub(super) fn enum_of(&self, e: &Expr) -> Option<usize> {
        if matches!(e.kind, ExprKind::This) {
            return (self.this_form == ThisForm::Enum).then_some(self.this_struct);
        }
        self.enum_type(e.ty)
    }

    /// The concrete classes whose objects the enum of type `t` holds, in `Class::types` order.
    pub(super) fn enum_variants(&self, t: usize) -> Vec<usize> {
        if let Some(found) = self.enum_variant_lists.borrow().get(&t) {
            return found.to_vec();
        }
        // An enum's constants exist without `new`.
        let found: Vec<usize> =
            self.variants(t).iter().copied().filter(|&c| self.live[c] || self.holds_value(c)).collect();
        self.enum_variant_lists.borrow_mut().insert(t, found.clone().into());
        found
    }

    /// The path of the variant of class `c` in the enum of `t`: `Shape::Circle`. A variant is
    /// named after its class, with the class's outer classes when two variants share a name, and
    /// with its package too when two classes of different packages share that
    /// (`JavaUtilStreamAbstractSetInEntrySet`).
    pub(super) fn variant(&self, t: usize, c: usize) -> String {
        self.need(Need::Enum(t));
        let names = self.variant_names_of(t);
        let name = match names.get(&c) {
            Some(name) => name.clone(),
            None => self.variant_name(&self.variants(t), c),
        };
        format!("{}::{name}", struct_name(&self.class.types[t].name))
    }

    /// The path of the variant of class `c` in the view of the enum of `t` (see `thin_impl`), as
    /// a pattern matches it: `ShapeView::Circle`.
    pub(super) fn view_pattern(&self, t: usize, c: usize) -> String {
        let variant = self.variant(t, c);
        let short = variant.rsplit("::").next().unwrap_or_default();
        format!("{}::{short}", self.view_name(t))
    }

    /// The name of the view of the enum of `t`: its name and `View`, or `Variants` where the
    /// program has a class of that name in the package.
    pub(super) fn view_name(&self, t: usize) -> String {
        let java = &self.class.types[t].name;
        let taken = [format!("{java}View"), format!("{java}.View")]
            .iter()
            .any(|n| self.class.type_index(n).is_some());
        let name = struct_name(java);
        if taken { name + "Variants" } else { name + "View" }
    }

    /// The number of class `c` in a `java::Thin`: the same in the enum of every type, so that a
    /// value keeps its bits as a value of a supertype (see `class_numbers`).
    pub(super) fn class_tag(&self, c: usize) -> u16 {
        self.class_tags[c]
    }

    /// Whether the variant of class `c` holds a handle (`java::Thin::pointer`), else a value
    /// (`java::Thin::value`): an object, an array, a builder, a box that keeps its identity or a
    /// string that does.
    pub(super) fn thin_pointer(&self, c: usize) -> bool {
        match self.runtime_of(c) {
            Some(Type::Array(_) | Type::StringBuilder) => true,
            // A string with an identity is the address of its text (`java::JStr`).
            Some(Type::String) => super::shared_strings(),
            Some(ty @ Type::Boxed(_)) => self.faithful_box(ty),
            Some(_) => false,
            None => !self.holds_value(c),
        }
    }

    /// The class numbers of `classes` as the pattern of a `match` on the tag of a value whose
    /// class is one of `among` (which holds `classes`): ranges, `3..=7 | 9`. No value has a number
    /// that no class of `among` has, so a range spans each run of `classes` that no other class of
    /// `among` breaks. The classes at or below a class are one run (`class_numbers`), and so are
    /// those of an interface that happen to be numbered together: a range is one subtract and one
    /// compare, where a list is a compare per number or a table.
    pub(super) fn tags(&self, classes: &[usize], among: &[usize]) -> String {
        let mut numbers: Vec<(u16, bool)> =
            among.iter().map(|&c| (self.class_tag(c), classes.contains(&c))).collect();
        numbers.extend(classes.iter().map(|&c| (self.class_tag(c), true)));
        // A class of both lists has the same entry in each.
        numbers.sort_unstable();
        numbers.dedup();
        let mut runs: Vec<(u16, u16)> = Vec::new();
        let mut open = false;
        for (tag, passes) in numbers {
            match runs.last_mut() {
                Some(run) if open && passes => run.1 = tag,
                _ if passes => runs.push((tag, tag)),
                _ => {}
            }
            open = passes;
        }
        let patterns: Vec<String> = runs
            .into_iter()
            .map(|(first, last)| if first == last { first.to_string() } else { format!("{first}..={last}") })
            .collect();
        patterns.join(" | ")
    }

    /// The constructor of each variant (`Shape::Circle(handle)`, an associated function named as
    /// the variant was) and `view()`, which lends what the value holds as the view's variant
    /// (`java::Thin`).
    fn thin_impl(&mut self, name: &str, view: &str, variants: &[usize], shorts: &[String]) {
        let vis = self.vis();
        self.gap();
        self.line(&format!("impl {name} {{"));
        self.indent += 1;
        for (&c, short) in variants.iter().zip(shorts) {
            let payload = self.payload(c);
            let (tag, make) = (self.class_tag(c), if self.thin_pointer(c) { "pointer" } else { "value" });
            self.line("#[allow(non_snake_case)]");
            self.line("#[inline]");
            self.line(&format!("{vis}fn {short}(o: {payload}) -> {name} {{"));
            self.line(&format!("    {name}(java::Thin::{make}({tag}, o))"));
            self.line("}");
        }
        self.line("#[inline]");
        if variants.is_empty() {
            self.line(&format!("{vis}fn view(&self) -> {view} {{"));
            self.line("    java::bad_tag(self.0.tag())");
            self.line("}");
        } else {
            self.line(&format!("{vis}fn view(&self) -> {view}<'_> {{"));
            self.indent += 1;
            self.line("match self.0.tag() {");
            self.indent += 1;
            for (&c, short) in variants.iter().zip(shorts) {
                let (tag, lend) =
                    (self.class_tag(c), if self.thin_pointer(c) { "as_pointer" } else { "as_value" });
                // The class number is the one its constructor gives.
                self.line(&format!("{tag} => {view}::{short}(unsafe {{ self.0.{lend}() }}),"));
            }
            self.line("tag => java::bad_tag(tag),");
            self.indent -= 1;
            self.line("}");
            self.indent -= 1;
            self.line("}");
        }
        self.indent -= 1;
        self.line("}");
    }

    /// `hierarchy::rep`, worked out once for each type.
    pub(super) fn rep(&self, t: usize) -> hierarchy::Rep {
        if let Some(&rep) = self.reps.borrow().get(&t) {
            return rep;
        }
        let rep = hierarchy::rep(self.class, self.facts, t);
        self.reps.borrow_mut().insert(t, rep);
        rep
    }

    /// `hierarchy::variants`, worked out once for each type.
    pub(super) fn variants(&self, t: usize) -> std::rc::Rc<Vec<usize>> {
        if let Some(variants) = self.variant_lists.borrow().get(&t) {
            return variants.clone();
        }
        let variants = std::rc::Rc::new(hierarchy::variants(self.class, self.facts, t));
        self.variant_lists.borrow_mut().insert(t, variants.clone());
        variants
    }

    /// The names of the variants of the enum of `t`, made once for all of them.
    fn variant_names_of(&self, t: usize) -> std::rc::Rc<HashMap<usize, String>> {
        if let Some(names) = self.variant_names.borrow().get(&t) {
            return names.clone();
        }
        let variants = self.variants(t);
        let count = |name: &dyn Fn(usize) -> String| {
            let mut counts: HashMap<String, usize> = HashMap::new();
            for &o in variants.iter() {
                *counts.entry(name(o)).or_default() += 1;
            }
            counts
        };
        let (simple, in_package) = (count(&|c| self.simple_name(c)), count(&|c| self.in_package_name(c)));
        let mut names: HashMap<usize, String> = variants
            .iter()
            .map(|&c| {
                let name = if simple[&self.simple_name(c)] <= 1 {
                    self.simple_name(c)
                } else if in_package[&self.in_package_name(c)] <= 1 {
                    self.in_package_name(c)
                } else {
                    self.full_variant_name(c)
                };
                (c, name)
            })
            .collect();
        // One class's simple name can be another's name in its package (`ClassReader.ProxyType`
        // and `Proxy.Type`, both `ProxyType`): such variants take their whole names.
        let mut seen: HashMap<String, usize> = HashMap::new();
        for &c in variants.iter() {
            *seen.entry(names[&c].clone()).or_default() += 1;
        }
        for &c in variants.iter() {
            if seen[&names[&c]] > 1 {
                names.insert(c, self.full_variant_name(c));
            }
        }
        // Whole names can still meet in one package (`CSS.Value` and `CSSValue`): all but the
        // first such variant take the first number after the name that no variant has.
        let mut taken: HashSet<String> = names.values().cloned().collect();
        let mut first: HashSet<String> = HashSet::new();
        for &c in variants.iter() {
            let name = names[&c].clone();
            if first.insert(name.clone()) {
                continue;
            }
            let numbered =
                (2..).map(|n| format!("{name}{n}")).find(|n| !taken.contains(n)).unwrap_or_default();
            taken.insert(numbered.clone());
            names.insert(c, numbered);
        }
        let names = std::rc::Rc::new(names);
        self.variant_names.borrow_mut().insert(t, names.clone());
        names
    }

    /// The name of the variant for class `c` among `variants`: its simple name, unless another
    /// variant shares it, then its name in its package, then its whole name.
    fn variant_name(&self, variants: &[usize], c: usize) -> String {
        let shared = |name: &dyn Fn(usize) -> String| {
            let own = name(c);
            variants.iter().filter(|&&o| name(o) == own).count() > 1
        };
        if !shared(&|c| self.simple_name(c)) {
            self.simple_name(c)
        } else if !shared(&|c| self.in_package_name(c)) {
            self.in_package_name(c)
        } else {
            self.full_variant_name(c)
        }
    }

    fn in_package_name(&self, c: usize) -> String {
        let (element, arrays) = self.element_name(c);
        modules::split(element).1.replace(['.', '$'], "") + &arrays
    }

    fn full_variant_name(&self, c: usize) -> String {
        let (element, arrays) = self.element_name(c);
        element.split(['.', '$']).map(capitalized).collect::<String>() + &arrays
    }

    /// The name of type `t`, or of its elements' class for an array type, with `Array` for each
    /// dimension: `Bucket[]` is named as `Bucket` and `Array`.
    fn element_name(&self, t: usize) -> (&str, String) {
        let name = &self.class.types[t].name;
        if self.class.array_of(t).is_none() {
            return (name, String::new());
        }
        let element = name.trim_end_matches("[]");
        (element, "Array".repeat((name.len() - element.len()) / 2))
    }

    /// `variant`, when the enum of `t` has a variant for class `c`: it has none when no object of
    /// `c` is ever created, where code that has one never runs.
    pub(super) fn variant_of(&self, t: usize, c: usize) -> Option<String> {
        self.enum_variants(t).contains(&c).then(|| self.variant(t, c))
    }

    /// The name of the conversion to the enum of `t`: `to_shape`.
    pub(super) fn to_name(&self, t: usize) -> String {
        format!("to_{}", self.conversion_names[t])
    }

    /// The name of each type in the names of its conversions (`as_name`, `to_name`), in snake
    /// case: its simple name, or with its outer classes when another type has that simple name
    /// too (`image_reader_node` and `concurrent_hash_map_node`, or `array_cache_byte_bucket_array`
    /// for an array), or with its package as well when that is shared too. A name with outer
    /// classes counts as shared when it is another type's simple name (`CSS.Value` beside
    /// `CSSValue`). Where two types still come to one name in snake case (JNA's `WinDef.LONG`
    /// and `Long`), all but the one with the fewest dots in its name (and of those, the first by
    /// name) take the first number after it that no type's name is: `long_2`.
    pub(super) fn conversion_names(&self) -> Vec<String> {
        let types = 0..self.class.types.len();
        let mut seen = HashSet::new();
        let mut shared = HashSet::new();
        for t in types.clone() {
            let names: BTreeSet<String> = [self.simple_name(t), self.in_package_name(t)].into();
            for name in names {
                if !seen.insert(name.clone()) {
                    shared.insert(name);
                }
            }
        }
        let distinct = |t: usize| {
            let simple = self.simple_name(t);
            if !shared.contains(&simple) {
                return simple;
            }
            let in_package = self.in_package_name(t);
            if !shared.contains(&in_package) {
                return in_package;
            }
            self.full_variant_name(t)
        };
        let mut names: Vec<String> =
            types.map(|t| snake_case(&distinct(t)).trim_end_matches('_').to_string()).collect();
        let mut by_name: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (t, name) in names.iter().enumerate() {
            by_name.entry(name).or_default().push(t);
        }
        let taken: HashSet<String> = by_name.keys().map(|n| n.to_string()).collect();
        let mut renamed = Vec::new();
        for (name, types) in by_name.iter().filter(|(_, types)| types.len() > 1) {
            let mut types = types.clone();
            let full = |t: &usize| &self.class.types[*t].name;
            types.sort_by_key(|t| (full(t).matches('.').count(), full(t).clone()));
            let mut free = (2..).map(|n| format!("{name}_{n}")).filter(|f| !taken.contains(f));
            for t in &types[1..] {
                renamed.push((*t, free.next().expect("a free name")));
            }
        }
        for (t, name) in renamed {
            names[t] = name;
        }
        names
    }

    /// The simple name of type `t`, which names its variants and conversions: `Circle` for
    /// `shapes.Circle`, `IntArray` for the arrays `int[]` that become `Object`s.
    fn simple_name(&self, t: usize) -> String {
        if self.class.array_of(t).is_some() {
            let (element, arrays) = self.element_name(t);
            return capitalized(element.rsplit(['.', '$']).next().unwrap_or_default()) + &arrays;
        }
        self.class.types[t].name.rsplit(['.', '$']).next().unwrap_or_default().to_string()
    }

    /// The name of the checked conversion to type `t`: `as_circle`.
    pub(super) fn as_name(&self, t: usize) -> String {
        format!("as_{}", self.conversion_names[t])
    }

    /// The binary name of type `t`, as the JVM shows it: `shapes.Outer$Inner`.
    pub(super) fn binary_name(&self, t: usize) -> String {
        let decl = &self.class.types[t];
        if let Some(array) = self.class.array_of(t) {
            return self.descriptor(array);
        }
        if decl.kind == TypeKind::Runtime {
            return decl.name.clone();
        }
        if let Some((binary, _)) = decl.origin.java_names() {
            return binary.to_string();
        }
        // The types of `java.lang` that lowering adds are named without their package.
        if decl.package.is_empty()
            && matches!(decl.name.as_str(), OBJECT | COMPARABLE | ENUM | NUMBER | CHAR_SEQUENCE | RECORD)
        {
            return format!("java.lang.{}", decl.name);
        }
        match decl.name.strip_prefix(&decl.package).and_then(|rest| rest.strip_prefix('.')) {
            Some(rest) if !decl.package.is_empty() => format!("{}.{}", decl.package, rest.replace('.', "$")),
            _ => decl.name.replace('.', "$"),
        }
    }

    /// The type of the program that a class or enum type names.
    pub(super) fn type_of(&self, ty: Type) -> Option<usize> {
        self.class.type_of(ty)
    }

    /// The Java type of the values of type `t`.
    fn java_type(&self, t: usize) -> Type {
        let decl = &self.class.types[t];
        match decl.kind {
            TypeKind::Enum => Type::enum_type(&decl.name),
            TypeKind::Runtime => self.runtime_of(t).expect("a runtime class"),
            _ => Type::class(&decl.name),
        }
    }

    /// How the value of `value`, of a class or enum type, becomes a value of class or enum type
    /// `to`.
    pub(super) fn narrowing(&self, value: &Expr, to: Type) -> Narrowing {
        let h = &self.class.hierarchy;
        let (Some(s), Some(t)) = (self.type_of(value.ty), self.type_of(to)) else { return Narrowing::Up };
        if h.is_subtype(s, t) {
            return Narrowing::Up;
        }
        match self.enum_of(value) {
            Some(e) => Narrowing::Down(e),
            None => Narrowing::Known(h.is_subtype(self.value_class(value), t)),
        }
    }

    /// The array class that `value`, held as an `Object`, is tested against where the translation
    /// holds arrays of it as `Object[]`s (`Class::array_tests`): the code of the class, and its
    /// name and module as a `ClassCastException` gives them.
    fn tested_array(&mut self, value: &Expr) -> Option<(String, String)> {
        let exact = *self.class.array_tests.get(&value.id)?;
        let class = self.type_class(exact);
        let name = rust_string(&self.descriptor(exact));
        let module = self.type_module(exact);
        Some((class, format!("({name}, {module})")))
    }

    /// Whether `value` may be a class that Java lets be a `to`, one of the interfaces that
    /// `java.lang.Class` implements and the program translates as interfaces of its own, which
    /// the runtime's classes do not implement (`Constable`, `TypeDescriptor.OfField`; `Type`,
    /// `TypeDescriptor`, `GenericDeclaration` and `Serializable` they do, see
    /// `CLASS_VALUE_INTERFACES`): the cast or the test is rejected.
    fn rejects_class_as(&mut self, value: &Expr, to: Type) -> bool {
        let Type::Class(name, _) = to else { return false };
        if !crate::ast::class_only_interface(name) {
            return false;
        }
        let holds_class = value.ty == Type::JavaClass
            || self.type_of(value.ty).is_some_and(|s| {
                self.enum_variants(s).into_iter().any(|c| self.runtime_of(c) == Some(Type::JavaClass))
            });
        if holds_class {
            let message = format!("a `java.lang.Class` as a `{name}` not supported yet");
            self.rejected.push(Error { pos: value.pos, message });
        }
        holds_class
    }

    /// The variants of the enum of `s` whose objects are of type `t`.
    fn passing(&self, s: usize, t: usize) -> Vec<usize> {
        self.enum_variants(s).into_iter().filter(|&c| self.class.hierarchy.is_subtype(c, t)).collect()
    }

    /// The text of the value of `value` for a method to be called on it in a condition: a local
    /// or `this` as it is, anything else in a block of its own, so that no borrow it makes
    /// outlives the test (a let chain keeps its temporaries alive through the guarded block).
    fn tested(&mut self, value: &'a Expr, text: String) -> String {
        match self.peel(value).kind {
            ExprKind::Var(_) | ExprKind::This => text,
            _ => format!("{{ let value = {text}; value }}"),
        }
    }

    /// `(T) value`, a checked downcast, or a cast across, from the enum of `s`; `checked` when
    /// a `catch` or a `finally` sees its failure (see `checked_class_cast`).
    fn downcast(&mut self, s: usize, to: Type, value: &'a Expr, checked: Option<ExprId>) -> Code {
        let t = self.type_of(to).expect("a cast to a type of the program");
        // No class of the enum is a `T` (no object of one is created): only null passes.
        if self.passing(s, t).is_empty() {
            return self.failing_cast(to, value, checked);
        }
        self.need(Need::Narrow(s, t));
        self.need(Need::CastSource(s));
        let name = self.as_name(t);
        let cast = self.runtime(if checked.is_some() { "try_cast" } else { "cast" });
        let mut target = self.cast_class(t);
        // A cast to an array class that arrays held as `Object[]`s may not be, as `String[]`: the
        // `Object[]`, then the class it carries.
        let array = self.tested_array(value);
        if let Some((_, exact)) = &array {
            target = exact.clone();
        }
        let check = |g: &mut Self, call: String| match &array {
            Some((class, exact)) if checked.is_some() => {
                format!("{call}.and_then(|a| {}(a, {class}, {exact}))", g.runtime("try_cast_array"))
            }
            Some((class, exact)) => format!("{}({call}, {class}, {exact})", g.runtime("cast_array")),
            None => call,
        };
        let text = if self.nullable(value) {
            let option = self.object_place(value);
            let enum_name = struct_name(&self.class.types[s].name);
            let call = check(self, format!("{cast}(o, {enum_name}::{name}, {target})"));
            format!("{option}.as_ref().map(|o| {call})")
        } else {
            let obj = self.object(value);
            let obj = super::class_values::borrow_of(&obj);
            check(self, format!("{cast}({obj}, |o| o.{name}(), {target})"))
        };
        self.cast_result(text, checked, self.nullable(value))
    }

    /// A cast of a value whose class is known to a type that it is not of, which always fails
    /// (unless the value is null).
    fn failing_cast(&mut self, to: Type, value: &'a Expr, checked: Option<ExprId>) -> Code {
        let t = self.type_of(to).expect("a cast to a type of the program");
        let target = self.cast_class(t);
        let ty = self.ty_name(to);
        // The class of the object `o`: its variant's, or the one class it can be.
        let (from, binding) = match self.enum_of(value) {
            Some(s) => {
                self.need(Need::CastSource(s));
                (format!("{}::cast_class(&o)", self.runtime("CastSource")), "o")
            }
            None => (self.cast_class(self.value_class(value)), "_"),
        };
        if checked.is_some() {
            let fail = format!("Err::<{ty}, _>({}({from}, {target}))", self.runtime("cast_exception"));
            let text = if self.nullable(value) {
                format!("{}.as_ref().map(|{binding}| {fail})", self.object_place(value))
            } else {
                format!("{{ let {binding} = &{}; {fail} }}", self.expr(value).at(UNARY))
            };
            return self.cast_result(text, checked, self.nullable(value));
        }
        let failed = format!("{}({from}, {target})", self.runtime("cast_failed"));
        if self.nullable(value) {
            let option = self.object_place(value);
            return Code::atom(format!("{option}.as_ref().map(|{binding}| -> {ty} {{ {failed} }})"));
        }
        // The value is typed, as `cast_failed` returns `!` and a local it initializes may be used.
        let v = self.expr(value).at(UNARY);
        Code::atom(format!("{{ let {binding} = &{v}; None::<{ty}>.unwrap_or_else(|| {failed}) }}"))
    }

    /// `value instanceof T`.
    pub(super) fn instance_test(&mut self, value: &'a Expr, to: Type) -> Code {
        if self.rejects_class_as(value, to) {
            return Code::atom("false");
        }
        let nullable = self.nullable(value);
        let known = |g: &mut Self, passes: bool| {
            if nullable && passes {
                return Code::atom(format!("{}.is_some()", g.object_place(value)));
            }
            let text = passes.to_string();
            if matches!(g.peel(value).kind, ExprKind::Var(_) | ExprKind::This) {
                return Code::atom(text);
            }
            let v = g.expr(value).text;
            Code::new(format!("{{ let _ = {v}; {text} }}"), LOWEST)
        };
        match self.narrowing(value, to) {
            Narrowing::Up => known(self, true),
            Narrowing::Known(passes) => known(self, passes),
            Narrowing::Down(s) => {
                let t = self.type_of(to).expect("a test of a type of the program");
                let passing = self.passing(s, t);
                if passing.is_empty() {
                    return known(self, false);
                }
                if let Some((class, _)) = self.tested_array(value) {
                    self.need(Need::Narrow(s, t));
                    let name = self.as_name(t);
                    let is = format!("is_some_and(|a| {}(&a, {class}))", self.runtime("array_is"));
                    return Code::atom(match nullable {
                        true => {
                            format!("{}.as_ref().is_some_and(|o| o.{name}().{is})", self.object_place(value))
                        }
                        false => format!("{}.{name}().{is}", operand(&self.object(value))),
                    });
                }
                let tags = self.tags(&passing, &self.enum_variants(s));
                let text = if nullable {
                    format!(
                        "{}.as_ref().is_some_and(|o| matches!(o.0.tag(), {tags}))",
                        self.object_place(value)
                    )
                } else {
                    format!("matches!({}.0.tag(), {tags})", operand(&self.object(value)))
                };
                Code::atom(text)
            }
        }
    }

    /// The pattern that binds `name`, of type `ty`, from `source` (`pattern_source`):
    /// `Some(name)`. Where no value of the program is a `T` (`None::<T>`), the code that would run
    /// with it may still assign fields of it, as of a value of a class that no object of the
    /// program is: it is bound `mut` there, as it is where the method assigns the variable. A
    /// variable that the method may set to null is an `Option` like any other local, bound whole
    /// (`mut name @ Some(_)`).
    pub(super) fn pattern_binding(&self, name: &str, source: &str, ty: Type) -> String {
        let local = self.local(name);
        let assigned = assigns_any(&self.class.methods[self.method].body, name);
        if self.optional_var(self.method, ty, name) {
            return format!("mut {local} @ Some(_)");
        }
        if source.starts_with("None::<") || assigned {
            format!("Some(mut {local})")
        } else {
            format!("Some({local})")
        }
    }

    /// The `Option` that `value instanceof T name` binds `name` from: the value as a `T` when the
    /// test passes.
    pub(super) fn pattern_source(&mut self, value: &'a Expr, to: Type) -> String {
        let nullable = self.nullable(value);
        match self.narrowing(value, to) {
            Narrowing::Up | Narrowing::Known(true) => {
                let v = self.class_cast(to, value).text;
                if nullable { v } else { format!("Some({v})") }
            }
            Narrowing::Known(false) => format!("None::<{}>", self.ty_name(to)),
            Narrowing::Down(s) => {
                let t = self.type_of(to).expect("a test of a type of the program");
                if self.passing(s, t).is_empty() {
                    return format!("None::<{}>", self.ty_name(to));
                }
                self.need(Need::Narrow(s, t));
                let name = self.as_name(t);
                let mut text = if nullable {
                    let enum_name = struct_name(&self.class.types[s].name);
                    format!("{}.as_ref().and_then({enum_name}::{name})", self.object_place(value))
                } else {
                    format!("{}.{name}()", self.object(value))
                };
                if let Some((class, _)) = self.tested_array(value) {
                    text = format!("{text}.filter(|a| {}(a, {class}))", self.runtime("array_is"));
                }
                self.tested(value, text)
            }
        }
    }

    /// Whether the variant of `c` holds a value rather than a shared object of the program: the
    /// constant of an enum that implements the type, or a string or a box in an `Object`.
    pub(super) fn holds_value(&self, c: usize) -> bool {
        matches!(self.class.types[c].kind, TypeKind::Enum | TypeKind::Runtime)
    }

    /// Whether the variant of `c` holds the handle of an object of the program (see `payload`),
    /// which `java::Thin::as_prefix` may read.
    fn object_variant(&self, c: usize) -> bool {
        self.runtime_of(c).is_none() && !self.holds_value(c)
    }

    /// The string `s` (a `String`) as an `Object`.
    pub(super) fn string_as_object(&mut self, s: &str) -> String {
        let object = self.object_index().expect("an `Object[]` in a program that uses `Object`");
        let string = self.type_of(Type::String).expect("strings among `Object`'s runtime classes");
        format!("{}({s})", self.variant(object, string))
    }

    /// `StringArrayOf` and `ObjectArrayOf`: a copy of array `e` (an `Object[]` or a `String[]`) as
    /// an array of type `ty` (the other), each string cast back from its `Object` or put in one.
    pub(super) fn array_conversion(&mut self, builtin: Builtin, e: &'a Expr, ty: Type) -> Code {
        let from_nullable = self.nullable_element(crate::codegen::arrays::element(e.ty));
        let to_nullable = self.nullable_element(crate::codegen::arrays::element(ty));
        // Each element of the new array: from a plain element `s` (`plain`), or from an `Option`
        // (`optional`, an `Option` of the new element).
        let (plain, optional) = if builtin == Builtin::ObjectArrayOf {
            (self.string_as_object("s.clone()"), format!("s.clone().map(|s| {})", self.string_as_object("s")))
        } else {
            let object = self.object_index().expect("an `Object[]` in a program that uses `Object`");
            let string = self.type_of(Type::String).expect("strings among `Object`'s runtime classes");
            self.need(Need::Narrow(object, string));
            self.need(Need::CastSource(object));
            let (cast, name) = (self.runtime("cast"), self.as_name(string));
            let target = self.cast_class(string);
            (
                format!("{cast}(&s, |o| o.{name}(), {target})"),
                format!("s.as_ref().map(|o| {cast}(o, |o| o.{name}(), {target}))"),
            )
        };
        let each = match (from_nullable, to_nullable) {
            (false, false) => plain,
            (false, true) => format!("Some({plain})"),
            (true, true) => optional,
            (true, false) => format!("{}({optional})", self.runtime("non_null")),
        };
        let maker = self.array_maker("from_vec", ty);
        // A null array's copy is null.
        if self.nullable(e) {
            let option = self.object_place(e);
            return Code::atom(format!(
                "{option}.as_ref().map(|a| {maker}((&a.borrow()).iter().map(|s| {each}).collect::<Vec<_>>()))"
            ));
        }
        let slice = self.slice_of(e);
        Code::atom(format!("{maker}(({slice}).iter().map(|s| {each}).collect::<Vec<_>>())"))
    }

    /// `System.arraycopy(src, srcPos, dest, destPos, length)` where `src` and `dest` are
    /// `Object`s: the copy between their arrays when both are arrays of one type, else Java's
    /// `ArrayStoreException`. The positions are computed first; a null `src` or `dest` throws
    /// `NullPointerException`. Where a `catch` or a `finally` sees the copy's failure
    /// (`Facts::raising`), the checked form, whose failure goes on from here.
    pub(super) fn object_array_copy(&mut self, args: &'a [Expr], id: ExprId) -> Code {
        let [src, src_pos, dest, dest_pos, length] = args else {
            unreachable!("arraycopy has five arguments")
        };
        let ints: Vec<String> =
            [src_pos, dest_pos, length].iter().map(|e| self.value(e, Type::Int).text).collect();
        let (s, d) = (self.object(src), self.object(dest));
        let checked = self.facts.raising.contains(&id);
        // Names that no local of the method has, as `src` and `dest` may read one.
        let [p, q, n] = ["src_pos", "dest_pos", "length"].map(|base| self.temp(base));
        let arms = match self.object_index() {
            Some(t) => {
                let variants = self.enum_variants(t);
                let call = CopyCall { checked, names: [p.as_str(), q.as_str(), n.as_str()] };
                self.object_copy_arms(t, &variants, call)
            }
            None => vec!["_ => unreachable!(),".to_string()],
        };
        let code = Code::atom(format!(
            "{{ let ({p}, {q}, {n}) = ({}, {}, {}); match (&{s}, &{d}) {{ (src, dest) => match (src.view(), dest.view()) {{ {} }} }} }}",
            ints[0],
            ints[1],
            ints[2],
            arms.join(" ")
        ));
        if !checked {
            return code;
        }
        Code::atom(self.passed_on(code))
    }

    /// The arrays that become `Object`s: the variant of `Object` of each and its pattern, with
    /// its type.
    fn object_arrays(&self) -> Vec<(String, String, Type)> {
        let Some(t) = self.object_index() else { return Vec::new() };
        let mut out = Vec::new();
        for c in self.enum_variants(t) {
            if let Some(array) = self.class.array_of(c) {
                out.push((self.variant(t, c), self.view_pattern(t, c), array));
            }
        }
        out
    }

    /// A value that may be null, as an owned `Option` of it.
    fn owned_option(&mut self, e: &'a Expr) -> String {
        let text = self.expr(e).text;
        if self.nullable(e) { text } else { format!("Some({text})") }
    }

    /// `code`, the `Result` of a native of `java.lang.reflect.Array`, passed on where a handler
    /// sees its failure (`facts.raising`, `null_checked`), else thrown where it fails.
    fn reflect_result(&mut self, code: Code, id: ExprId) -> Code {
        if self.facts.raising.contains(&id) || self.facts.null_checked.contains_key(&id) {
            // The checked form raises the `NullPointerException` of a null argument too.
            self.dereferenced.insert(id);
            return Code::atom(self.passed_on(code));
        }
        let throw = self.runtime("throw_exception");
        Code::atom(format!("{}.unwrap_or_else(|e| {throw}(e))", code.at(ATOM)))
    }

    /// `Array.newArray(componentType, length)`: a new array of the component type among the
    /// arrays that become `Object`s, found by the type's name. `void` has no arrays (an
    /// `IllegalArgumentException`); a type of which the program makes no array as an `Object`
    /// ends the program, as no value of the translation can be that array.
    pub(super) fn reflect_new_array(&mut self, args: &'a [Expr], id: ExprId) -> Code {
        let [class, length] = args else { unreachable!("newArray has two arguments") };
        let class_text = self.owned_option(class);
        let length = self.value(length, Type::Int).text;
        let exception = self.runtime("Exception");
        let non_null = self.runtime("try_non_null");
        let object = self.ty_name(Type::class(OBJECT));
        let [c, n] = ["component", "length"].map(|base| self.temp(base));
        let mut arms = Vec::new();
        let mut names = Vec::new();
        let object_array = Type::array(Type::class(OBJECT));
        let mut objects = None;
        for (variant, _, array) in self.object_arrays() {
            let element = array.element();
            let name = rust_string(&self.type_class_name(element));
            let zero = self.element_zero(element);
            let filled =
                self.runtime(if self.shared_array_type(array) { "try_shared_filled" } else { "try_filled" });
            arms.push(format!("{name} => {filled}({n}, {zero}).map({variant}),"));
            if array == object_array {
                objects = Some((variant, filled, zero));
            }
            names.push(name);
        }
        // An array of a narrower class that the translation holds as an `Object[]` carrying its
        // class (`lower::covariant`), of the classes the code makes or tests arrays of.
        if let Some((variant, filled, zero)) = objects {
            let mut covariant: Vec<Type> = Vec::new();
            // In the order of the expressions, so that the translation is the same every time.
            let in_order = |map: &'a std::collections::HashMap<ExprId, Type>| {
                let mut found: Vec<(ExprId, Type)> = map.iter().map(|(&id, &ty)| (id, ty)).collect();
                found.sort_unstable_by_key(|&(id, _)| id);
                found.into_iter().map(|(_, ty)| ty)
            };
            for class in in_order(&self.class.element_classes).chain(in_order(&self.class.array_tests)) {
                if class != object_array && !covariant.contains(&class) {
                    covariant.push(class);
                }
            }
            for class in covariant {
                let name = rust_string(&self.type_class_name(class.element()));
                if names.contains(&name) {
                    continue;
                }
                let tagged = self.with_element_class(class, Code::atom("a".to_string())).text;
                arms.push(format!("{name} => {filled}({n}, {zero}).map(|a| {variant}({tagged})),"));
                names.push(name);
            }
        }
        arms.push(format!("\"void\" => Err({exception}::bare(\"java.lang.IllegalArgumentException\")),"));
        let missing = self.runtime("not_translated");
        arms.push(format!(
            "name => {missing}(&format!(\"`Array.newInstance` of `{{name}}`, of which the program makes no array as an object\")),"
        ));
        let code = Code::atom(format!(
            "{{ let ({c}, {n}) = ({class_text}, {length}); (|| -> Result<{object}, {exception}> {{ \
             match &*{non_null}({c})?.get_name().to_string_lossy() {{ {} }} }})() }}",
            arms.join(" ")
        ));
        self.reflect_result(code, id)
    }

    /// `Array.getLength(array)`.
    pub(super) fn reflect_array_length(&mut self, args: &'a [Expr], id: ExprId) -> Code {
        let [array] = args else { unreachable!("getLength has one argument") };
        let array_text = self.owned_option(array);
        let exception = self.runtime("Exception");
        let non_null = self.runtime("try_non_null");
        let object = self.ty_name(Type::class(OBJECT));
        let len = self.runtime("len");
        let a = self.temp("array");
        let mut arms: Vec<String> = self
            .object_arrays()
            .into_iter()
            .map(|(_, pattern, _)| format!("{pattern}(a) => Ok({len}(a)),"))
            .collect();
        arms.push(format!(
            "_ => Err({exception}::new(\"java.lang.IllegalArgumentException\", \"Argument is not an array\")),"
        ));
        let code = Code::atom(format!(
            "{{ let {a} = {array_text}; (|| -> Result<i32, {exception}> {{ match {object}::view({non_null}({a}.as_ref())?) {{ {} }} }})() }}",
            arms.join(" ")
        ));
        self.reflect_result(code, id)
    }

    /// `Array.set(array, index, value)`, checked in the JVM's order: a null array, one that is
    /// not an array, a null for a primitive element, an index outside the array, then a value
    /// that the element cannot hold (a box of a primitive that does not widen to it, or an object
    /// of another class).
    pub(super) fn reflect_array_set(&mut self, args: &'a [Expr], id: ExprId) -> Code {
        let [array, index, value] = args else { unreachable!("set has three arguments") };
        let array_text = self.owned_option(array);
        let index = self.value(index, Type::Int).text;
        let value_text = self.owned_option(value);
        let exception = self.runtime("Exception");
        let non_null = self.runtime("try_non_null");
        let object_type = self.ty_name(Type::class(OBJECT));
        let (len, set) = (self.runtime("len"), self.runtime("try_set"));
        let [a, i, v] = ["array", "index", "value"].map(|base| self.temp(base));
        let fail = |message: &str| match message {
            "" => format!("return Err({exception}::bare(\"java.lang.IllegalArgumentException\"))"),
            _ => {
                format!("return Err({exception}::new(\"java.lang.IllegalArgumentException\", \"{message}\"))")
            }
        };
        let bounds = format!(
            "if {i} < 0 || {i} >= {len}(a) {{ return Err({exception}::bare(\"java.lang.ArrayIndexOutOfBoundsException\")); }}"
        );
        let object = self.object_index();
        let mut arms = Vec::new();
        for (_, pattern, array) in self.object_arrays() {
            let element = array.element();
            let stored = if element.is_numeric() || element == Type::Boolean {
                let mut sources = Vec::new();
                for &source in widening_sources(element) {
                    let boxed = Type::boxed(source);
                    let Some(c) = self.class.type_of(boxed) else { continue };
                    let Some(o) = object.filter(|&o| self.enum_variants(o).contains(&c)) else { continue };
                    let source_variant = self.view_pattern(o, c);
                    let read = if self.faithful_box(boxed) { "**b" } else { "*b" };
                    let read = if source == element {
                        read.to_string()
                    } else {
                        format!("{read} as {}", rust_type(element))
                    };
                    sources.push(format!("{source_variant}(b) => {read},"));
                }
                sources.push(format!("_ => {},", fail("argument type mismatch")));
                format!(
                    "{{ let Some(given) = {v}.as_ref() else {{ {} }}; {bounds} match {object_type}::view(given) {{ {} }} }}",
                    fail(""),
                    sources.join(" ")
                )
            } else {
                let mismatch = fail("array element type mismatch");
                let narrowed = match (element, object) {
                    (Type::Class(name, _), _) if name == OBJECT => {
                        Some("Some(o) => Some(o.clone()),".to_string())
                    }
                    (_, Some(o)) => match self.class.type_of(element) {
                        Some(t) if !self.passing(o, t).is_empty() => {
                            self.need(Need::Narrow(o, t));
                            let enum_name = struct_name(&self.class.types[o].name);
                            let name = self.as_name(t);
                            Some(format!(
                                "Some(o) => match {enum_name}::{name}(o) {{ Some(x) => Some(x), None => {mismatch} }},"
                            ))
                        }
                        _ => None,
                    },
                    _ => None,
                };
                let narrowed = narrowed.unwrap_or_else(|| format!("Some(_) => {mismatch},"));
                let reference = format!("match {v}.as_ref() {{ None => None, {narrowed} }}");
                // An array whose elements are never null (`analysis::nullness`), as no null is
                // stored in it.
                let reference = match self.nullable_element(element) {
                    true => reference,
                    false => format!("{}({reference})", self.runtime("non_null")),
                };
                format!("{{ {bounds} {reference} }}")
            };
            // An `Object[]` may be an array of another class (`covariant`, also one that
            // `Array.newInstance` made): one that cannot hold the value refuses it, after the
            // index is checked.
            let store = if element == Type::class(OBJECT) {
                format!(
                    "{}(a, {i}, element).map_err(|_| {exception}::new(\"java.lang.IllegalArgumentException\", \"array element type mismatch\"))",
                    self.runtime("try_checked_set")
                )
            } else {
                format!("{set}(a, {i}, element)")
            };
            arms.push(format!("{pattern}(a) => {{ let element = {stored}; {store} }}"));
        }
        arms.push(format!("_ => {{ {}; }}", fail("Argument is not an array")));
        // The value's type is spelled out, as a `null` (`None`) may first be read where its type
        // is not yet known.
        let code = Code::atom(format!(
            "{{ let ({a}, {i}, {v}): (_, _, Option<{object_type}>) = ({array_text}, {index}, {value_text}); \
             (|| -> Result<(), {exception}> {{ \
             match {object_type}::view({non_null}({a}.as_ref())?) {{ {} }} }})() }}",
            arms.join(" ")
        ));
        self.reflect_result(code, id)
    }

    /// The name of the class of type `ty`, as `Class.getName()` gives it: `int`,
    /// `java.lang.String`, `[I`.
    fn type_class_name(&self, ty: Type) -> String {
        match ty {
            _ if ty.is_numeric() || ty == Type::Boolean => ty.to_string(),
            Type::Array(_) => self.descriptor(ty),
            Type::JavaClass => CLASS_VALUE.to_string(),
            _ => {
                let descriptor = self.descriptor(ty);
                match descriptor.strip_prefix('L').and_then(|d| d.strip_suffix(';')) {
                    Some(name) => name.to_string(),
                    None => descriptor,
                }
            }
        }
    }

    /// Class `c` as a `ClassCastException` names it (`java::CastClass`): its binary name and its
    /// module, `("Main$Dog", java::APP)`.
    pub(super) fn cast_class(&mut self, c: usize) -> String {
        let name = rust_string(&self.binary_name(c));
        let module = self.cast_module(c);
        format!("({name}, {module})")
    }

    /// The module and class loader of class `c` as HotSpot names them with the class: the JDK's
    /// `java.base` (`java::BASE`) for the classes that the runtime holds, an array's element's,
    /// and for any other class its package's (`Class::module_labels`), else the unnamed module of
    /// the system class loader (`java::APP`).
    fn cast_module(&mut self, c: usize) -> String {
        if let Some(ty) = self.runtime_of(c).or_else(|| self.class.array_of(c)) {
            return self.type_module(ty);
        }
        let decl = &self.class.types[c];
        let jdk = matches!(decl.name.as_str(), OBJECT | COMPARABLE | ENUM | NUMBER | CHAR_SEQUENCE | RECORD);
        if jdk || decl.origin == Origin::Object {
            return self.runtime("BASE");
        }
        match self.class.module_labels.get(&decl.package) {
            Some(label) if label == "module java.base of loader 'bootstrap'" => self.runtime("BASE"),
            Some(label) => rust_string(label),
            None => self.runtime("APP"),
        }
    }

    /// `cast_module` of the class of the values of type `ty`; an array's is its element's, and
    /// a primitive's (in an array) `java.base`.
    pub(super) fn type_module(&mut self, ty: Type) -> String {
        match ty {
            Type::Array(element) => self.type_module(*element),
            Type::Class(..) | Type::Enum(_) => match self.class.type_of(ty) {
                Some(c) if self.runtime_of(c).is_none() => self.cast_module(c),
                _ => self.runtime("BASE"),
            },
            _ => self.runtime("BASE"),
        }
    }

    /// The JVM's name of an array class, as `getClass().getName()` gives it: `[I`,
    /// `[Ljava.lang.String;`, `[[Lshapes.Point;`.
    fn descriptor(&self, ty: Type) -> String {
        match ty {
            Type::Array(element) => format!("[{}", self.descriptor(*element)),
            Type::Int => "I".to_string(),
            Type::Long => "J".to_string(),
            Type::Double => "D".to_string(),
            Type::Float => "F".to_string(),
            Type::Byte => "B".to_string(),
            Type::Short => "S".to_string(),
            Type::Char => "C".to_string(),
            Type::Boolean => "Z".to_string(),
            Type::String => "Ljava.lang.String;".to_string(),
            Type::StringBuilder => "Ljava.lang.StringBuilder;".to_string(),
            Type::Boxed(_) => format!("Ljava.lang.{ty};"),
            Type::Class(name, _) if name == OBJECT => "Ljava.lang.Object;".to_string(),
            Type::Class(name, _) | Type::Enum(name) => match self.class.type_index(name) {
                Some(t) => format!("L{};", self.binary_name(t)),
                None => format!("L{name};"),
            },
            _ => ty.to_string(),
        }
    }

    /// The Java type of the values of runtime class `c`, if it is one: a string, a box, or the
    /// arrays of an array class.
    pub(super) fn runtime_of(&self, c: usize) -> Option<Type> {
        if let Some(shared) = self.class.shared_of(c) {
            return Some(shared);
        }
        if self.class.types[c].kind == TypeKind::Runtime && self.class.types[c].name == CLASS_VALUE {
            return Some(Type::JavaClass);
        }
        if self.class.types[c].kind == TypeKind::Runtime && self.class.types[c].name == CHARSET_VALUE {
            return Some(Type::Charset);
        }
        (self.class.types[c].kind == TypeKind::Runtime)
            .then(|| runtime_type(&self.class.types[c].name))
            .flatten()
    }

    /// The value that the variant of `c` bound to `o` holds, as a value of its own: `*o` for an
    /// enum's constant or a plain box, `o.clone()` for an object or a string.
    fn held(&self, c: usize) -> &'static str {
        let copied = match self.runtime_of(c) {
            Some(ty) => {
                !matches!(ty, Type::String | Type::StringBuilder | Type::Array(_)) && !self.faithful_box(ty)
            }
            None => self.holds_value(c),
        };
        if copied { "*o" } else { "o.clone()" }
    }

    /// The class of the object, the enum of the constant, or the runtime class of the string or
    /// box, that `e` is: the variant it becomes.
    pub(super) fn value_class(&self, e: &Expr) -> usize {
        match e.ty {
            Type::Class(..) => self.struct_class(e).expect("an object has a class type"),
            _ => self.class.type_of(e.ty).expect("a value of a type of the program or of the runtime"),
        }
    }

    /// The variant's class of `e` (see `value_class`) and its handle or value.
    fn variant_value(&mut self, e: &'a Expr) -> (usize, String) {
        let c = self.value_class(e);
        let text = match self.runtime_of(c) {
            Some(ty) => self.value(e, ty).text,
            None if self.holds_value(c) => self.expr(e).text,
            None => self.object_handle(e),
        };
        (c, text)
    }

    /// The Rust type of the object of concrete class `c` that its variant holds.
    fn payload(&mut self, c: usize) -> String {
        if let Some(ty) = self.runtime_of(c) {
            return self.ty_name(ty);
        }
        if self.holds_value(c) {
            return struct_name(&self.class.types[c].name);
        }
        let structure = self.struct_of(c);
        let (pointer, cell) = (self.pointer(c), self.cell(c));
        let name = &self.class.types[c].name;
        if self.celled(name) {
            self.celled_handle(c, &structure)
        } else if self.facts.immutable.contains(name) {
            format!("{pointer}<{structure}>")
        } else {
            format!("{pointer}<{cell}<{structure}>>")
        }
    }

    /// Whether the objects of the enum of `t` are reached without a `RefCell`: immutable, or
    /// celled. A family is immutable together, and celled together.
    pub(super) fn plain_enum(&self, t: usize) -> bool {
        let concretes = self.class.hierarchy.concretes(t);
        !concretes.is_empty()
            && concretes.iter().all(|&c| {
                let name = &self.class.types[c].name;
                self.facts.immutable.contains(name) || self.celled(name)
            })
    }

    /// The class whose fields struct `base()` of the enum of `t` borrows: `t`'s own, or the
    /// nearest superclass's.
    fn fields_root(&self, t: usize) -> Option<usize> {
        if self.facts.fields_structs.contains(&t) {
            Some(t)
        } else {
            hierarchy::embedded(self.facts, self.class, t)
        }
    }

    /// The path of field `name` of a value of the enum of `t` from what `base()` borrows.
    pub(super) fn enum_field_path(&self, t: usize, ty: Type, name: &str) -> String {
        match (self.fields_root(t), self.class.field_owner(ty, name)) {
            (Some(root), Some(owner)) => {
                hierarchy::base_path(self.facts, self.class, root, owner)
                    + &field_name(&self.class.types[owner], name)
            }
            (None, Some(owner)) => field_name(&self.class.types[owner], name),
            _ => snake_case(name),
        }
    }

    /// The borrow of the fields of a value of the enum of `t`: `.base()` or `.base_mut()`.
    pub(super) fn enum_borrow(&self, t: usize, mutable: bool) -> &'static str {
        // A celled object's fields are cells, written through `base()`.
        if mutable && !self.plain_enum(t) {
            self.need(Need::BaseMut(t));
            ".base_mut()"
        } else {
            self.need(Need::Base(t));
            ".base()"
        }
    }

    /// `e` as a value of the enum of `t`, a type at or above its own, for a method of the enum
    /// to be called on: as it is, converted from a subtype's enum, or an object wrapped in its
    /// variant.
    pub(super) fn as_enum(&mut self, t: usize, e: &'a Expr) -> String {
        match self.enum_of(e) {
            Some(s) if s == t => self.object(e),
            Some(s) => {
                self.need(Need::Convert(s, t));
                format!("{}.{}()", self.object(e), self.to_name(t))
            }
            None => {
                let (c, handle) = self.variant_value(e);
                match self.variant_of(t, c) {
                    Some(variant) => format!("{variant}({handle})"),
                    // The object is read first, which may throw.
                    None => {
                        let ty = struct_name(&self.class.types[t].name);
                        format!("{{ let _ = {handle}; None::<{ty}>.unwrap_or_else(|| {}) }}", absent(self, c))
                    }
                }
            }
        }
    }

    /// A conversion of an object to a supertype: the same object as a value of the supertype's
    /// enum. Where both types have the same representation it changes nothing.
    pub(super) fn class_cast(&mut self, to: Type, value: &'a Expr) -> Code {
        let checked = self.checked_cast.take();
        if matches!(value.kind, ExprKind::Null) {
            return Code::atom("None");
        }
        if self.rejects_class_as(value, to) {
            return Code::atom("unreachable!()");
        }
        // A string, a builder or a box as a value of a type of the program whose values do not
        // include it, which `lower::hierarchy::convert` leaves to the generator.
        if matches!(value.ty, Type::String | Type::StringBuilder | Type::Boxed(_))
            && !self
                .type_of(value.ty)
                .zip(self.type_of(to))
                .is_some_and(|(s, t)| self.class.hierarchy.is_subtype(s, t))
        {
            let message = format!("a `{}` as a `{to}` not supported yet", value.ty);
            self.rejected.push(Error { pos: value.pos, message });
            return Code::atom("unreachable!()");
        }
        match self.narrowing(value, to) {
            Narrowing::Down(s) => return self.downcast(s, to, value, checked),
            Narrowing::Known(false) => return self.failing_cast(to, value, checked),
            Narrowing::Up | Narrowing::Known(true) => {}
        }
        let Some(t) = self.enum_type(to) else {
            // The values of `to` are the objects of one class: a value of another class is one
            // whose objects never reach `to`, so it is null.
            let target = self.class.class_index(to).map(|t| self.rep_class(t));
            if let (Some(target), Some(c)) = (target, self.struct_class(value))
                && target != c
            {
                let ty = self.ty_name(to);
                let text = self.expr(value).text;
                return Code::atom(if self.nullable(value) {
                    format!("{{ let _ = {text}; None::<{ty}> }}")
                } else {
                    format!("{{ let _ = {text}; None::<{ty}>.unwrap_or_else(|| {}) }}", absent(self, c))
                });
            }
            return self.expr(value);
        };
        // A new object becomes a value of the target's enum directly.
        if let ExprKind::New { ctor: Some(ctor), args, .. } = &value.kind {
            return self.new_object_as(value.id, *ctor, args, Some(t));
        }
        let from = self.enum_of(value);
        if from == Some(t) {
            return self.expr(value);
        }
        if self.nullable(value) {
            let option = self.expr(value).receiver();
            return Code::atom(match from {
                Some(s) => {
                    self.need(Need::Convert(s, t));
                    format!(
                        "{option}.as_ref().map({}::{})",
                        struct_name(&self.class.types[s].name),
                        self.to_name(t)
                    )
                }
                None => {
                    let c = self.value_class(value);
                    match self.variant_of(t, c) {
                        // A borrowed optional string is an `Option<&str>`.
                        Some(variant) if matches!(&value.kind, ExprKind::Var(name) if self.borrowed_optional.contains(name)) =>
                        {
                            format!("{option}.map(|s| {variant}(s.to_owned()))")
                        }
                        Some(variant) => format!("{option}.map({variant})"),
                        None => {
                            format!("{option}.map(|_| -> {} {{ {} }})", self.ty_name(to), absent(self, c))
                        }
                    }
                }
            });
        }
        match from {
            Some(s) => {
                self.need(Need::Convert(s, t));
                Code::atom(format!("{}.{}()", self.object(value), self.to_name(t)))
            }
            None => {
                let c = self.value_class(value);
                // An object's last use moves its handle in.
                // No object of the class is created, so none reaches here; the value is still
                // computed first, which may throw.
                let Some(variant) = self.variant_of(t, c) else {
                    let ty = self.ty_name(to);
                    let text = self.expr(value).text;
                    return Code::atom(format!(
                        "{{ let _ = {text}; None::<{ty}>.unwrap_or_else(|| {}) }}",
                        absent(self, c)
                    ));
                };
                let v = match self.runtime_of(c) {
                    Some(_) => self.variant_value(value).1,
                    None => self.expr(value).text,
                };
                Code::atom(format!("{variant}({v})"))
            }
        }
    }

    /// `a == b` and `a != b` on values of an enum: identity, through `PartialEq`.
    pub(super) fn enum_identity(&mut self, op: BinOp, l: &'a Expr, r: &'a Expr) -> Code {
        let symbol = if op == BinOp::Ne { "!=" } else { "==" };
        let text = if self.nullable(l) || self.nullable(r) {
            let (a, b) = (self.enum_option(l), self.enum_option(r));
            format!("{a} {symbol} {b}")
        } else {
            let (a, b) = (self.object_ref(l), self.object_ref(r));
            format!("{a} {symbol} {b}")
        };
        Code::new(text, CMP)
    }

    /// A value of an enum as an `Option<&..>`: `None` for null.
    pub(super) fn enum_option(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Null => "None".to_string(),
            _ if self.nullable(e) => format!("{}.as_ref()", self.object_place(e)),
            _ => format!("Some({})", self.object_ref(e)),
        }
    }

    /// A call at `pos` of instance method `resolved` on a value of the enum of `t`: the method of
    /// the enum that runs what the object's class runs. `toString` is the enum's `Display`.
    pub(super) fn enum_call(
        &mut self,
        t: usize,
        pos: Pos,
        receiver: &'a Expr,
        resolved: usize,
        args: &'a [Expr],
    ) -> Code {
        let m = &self.class.methods[resolved];
        // A class of the program that has none of `CharSequence`'s methods (`lower` links its own
        // or the default `getChars`, so none should) rejects each call that may reach it, and
        // the dispatcher's arm for it never runs.
        if self.is_char_sequence_method(resolved)
            && self.enum_variants(t).into_iter().any(|c| self.missing_sequence_method(resolved, c))
        {
            let message = format!("`CharSequence.{}` of a class without one not supported yet", m.name);
            self.rejected.push(Error { pos, message });
            return Code::atom("unreachable!()".to_string());
        }
        if m.name == "toString" && m.params.is_empty() {
            self.need(Need::Display(t));
            let obj = self.object(receiver);
            // A `toString` that may throw is a `Result` here, whose error the caller passes on
            // (`raising`).
            let throws = self.facts.throwing.contains(&resolved);
            // One that an implementation may return null from is an `Option` where its value is
            // used (`nullness`).
            if self.facts.nullable_returns.contains(&resolved) {
                self.need(Need::NullableText(t));
                let text = format!("{obj}.nullable_text()");
                if throws || !self.has_throwable() {
                    return Code::atom(text);
                }
                let uncaught = self.uncaught("uncaught");
                return Code::atom(format!("{text}.unwrap_or_else({uncaught})"));
            }
            return Code::atom(match (throws, self.enum_text_throws(t)) {
                (true, true) => format!("{obj}.try_text()"),
                (true, false) => {
                    self.need(Need::JavaString(t));
                    format!("Ok::<_, {}>({obj}.java_string())", self.throwable_type())
                }
                (false, _) => {
                    self.need(Need::JavaString(t));
                    format!("{obj}.java_string()")
                }
            });
        }
        let name = match self.among(t, receiver) {
            Some(key) if self.enum_body(t, resolved).is_none() => {
                let group = self.class.hierarchy.group_id(resolved);
                self.need(Need::Among(t, group, key));
                format!("{}_among_{key:016x}", self.group_names[&group])
            }
            _ => self.dispatch_name(t, resolved),
        };
        let obj = self.object(receiver);
        let args = self.call_args(resolved, args).join(", ");
        Code::atom(format!("{obj}.{name}({args})"))
    }

    /// The key of the classes that `receiver`, a value of the enum of `t`, may be, where the
    /// points-to facts narrow them to a few of the enum's (`Class::receivers`): a call on it
    /// dispatches among those, in a dispatcher small enough to inline, with the whole one for any
    /// other class, so that a call whose receiver is of one class is a test of its number and a
    /// call the compiler can inline.
    fn among(&self, t: usize, receiver: &Expr) -> Option<u64> {
        let set = self.class.receivers.get(&receiver.id)?;
        let variants = self.enum_variants(t);
        let among = set.iter().filter(|c| variants.contains(c)).count();
        (among > 0 && among <= AMONG_MOST && among < variants.len()).then(|| receiver_key(set))
    }

    /// The implementations that the variants of the enum of `t` run for a call of `resolved`.
    fn enum_targets(&self, t: usize, resolved: usize) -> Vec<usize> {
        let mut out = Vec::new();
        for c in self.enum_variants(t) {
            if let Some(i) = self.class.hierarchy.implementation(resolved, c)
                && !out.contains(&i)
            {
                out.push(i);
            }
        }
        out
    }

    /// The name of the method of the enum of `t` that a call of `resolved` calls: the body
    /// itself when every variant that can receive the call runs the one the enum has, else the
    /// dispatcher. A variant outside the class that declares `resolved` cannot receive it
    /// (`Throwable`'s `Exception`s for `Error`'s `construct<Name>` methods, see
    /// `analysis::whole_objects`).
    pub(super) fn dispatch_name(&self, t: usize, resolved: usize) -> String {
        if let Some(only) = self.enum_body(t, resolved) {
            self.need(Need::Body(only));
            return self.fn_names[only].clone();
        }
        let group = self.class.hierarchy.group_id(resolved);
        self.need(Need::Dispatch(t, group));
        self.group_names[&group].clone()
    }

    /// The body that a call of `resolved` on the enum of `t` calls directly, when it calls no
    /// dispatcher (see `dispatch_name`).
    pub(super) fn enum_body(&self, t: usize, resolved: usize) -> Option<usize> {
        if let Some(&body) = self.enum_bodies.borrow().get(&(t, resolved)) {
            return body;
        }
        let body = self.find_enum_body(t, resolved);
        self.enum_bodies.borrow_mut().insert((t, resolved), body);
        body
    }

    /// `enum_body`, found.
    fn find_enum_body(&self, t: usize, resolved: usize) -> Option<usize> {
        let h = &self.class.hierarchy;
        let declared = self.class.methods[resolved].class;
        match self.enum_targets(t, resolved)[..] {
            [only]
                if self.homes[only] == Home::Enum(t)
                    && self.same_ret(only, resolved)
                    && self
                        .enum_variants(t)
                        .into_iter()
                        .all(|c| h.implementation(resolved, c).is_some() || !h.is_subtype(c, declared))
                    // A class answers `Type`'s `getTypeName()` and `TypeDescriptor`'s
                    // `descriptorString()` itself (see `class_answer`).
                    && !(self.class_answer(resolved).is_some()
                        && self.enum_variants(t).into_iter().any(|c| self.runtime_of(c) == Some(Type::JavaClass))) =>
            {
                Some(only)
            }
            _ => None,
        }
    }

    /// A call of method `index`, emitted on the enum of a type or on a fields struct, from code
    /// whose receiver is not the struct that has it: `Base::Circle(c.clone()).describe_in_base()`
    /// or, in a method of an enum, `self.base_mut().get_name()`. `None` for any other call.
    pub(super) fn inherited_call(
        &mut self,
        receiver: &'a Expr,
        index: usize,
        args: &'a [Expr],
    ) -> Option<Code> {
        match self.homes[index] {
            Home::Enum(d) => {
                self.need(Need::Body(index));
                let obj = self.as_enum(d, receiver);
                let args = self.call_args(index, args).join(", ");
                Some(Code::atom(format!("{obj}.{}({args})", self.fn_names[index])))
            }
            Home::Fields(d) => {
                let t = self.enum_of(receiver)?;
                let root = self.fields_root(t).expect("a fields struct is at or above its subclasses");
                let path = hierarchy::base_path(self.facts, self.class, root, d);
                let borrow = self.enum_borrow(t, self.facts.mutating.contains(&index));
                let name = self.fn_names[index].clone();
                // The object is borrowed before the arguments run, so arguments that may reach an
                // object of the class run first, as `bound_call` runs them.
                let Type::Class(class, _) = receiver.ty else { unreachable!("a receiver has a class type") };
                if args.iter().any(|a| has_effects(a) || reads_class(self.class, a, class)) {
                    let outer = std::mem::replace(&mut self.guarded, false);
                    let mut lines = Vec::new();
                    let obj = match &receiver.kind {
                        ExprKind::This | ExprKind::Var(_) => self.object(receiver),
                        _ => {
                            let temp = self.temp("object");
                            lines.push(format!("let {temp} = {};", self.object(receiver)));
                            temp
                        }
                    };
                    let texts = self.call_args(index, args);
                    let mut bound = Vec::new();
                    for (text, p) in texts.into_iter().zip(&self.class.methods[index].params) {
                        let mut temp = self.temp(&snake_case(&p.name));
                        while bound.contains(&temp) || temp == obj {
                            temp.push('_');
                        }
                        // A nullable string is bound as an owned value first, as `bound_call` does.
                        if let Some(owned) = text.strip_suffix(".as_deref()") {
                            lines.push(format!("let {temp}_value = {owned}.clone();"));
                            lines.push(format!("let {temp} = {temp}_value.as_deref();"));
                        } else {
                            lines.push(format!("let {temp} = {text};"));
                        }
                        bound.push(temp);
                    }
                    self.guarded = outer;
                    lines.push(format!("{obj}{borrow}.{path}{name}({})", bound.join(", ")));
                    return Some(Code::atom(format!("{{ {} }}", lines.join(" "))));
                }
                let obj = self.object(receiver);
                let args = self.call_args(index, args).join(", ");
                Some(Code::atom(format!("{obj}{borrow}.{path}{}({args})", self.fn_names[index])))
            }
            _ => None,
        }
    }

    /// The text of a value of an enum, a `java::Piece` of a concatenation: the value itself,
    /// whose piece is its `java_string()`, where it is converted after the other operands, else
    /// that string.
    pub(super) fn enum_text(&mut self, t: usize, obj: String) -> String {
        self.need(Need::Display(t));
        if self.enum_text_throws(t) {
            // A `toString` that throws is called, through `try_text`.
            return self.passed_on(Code::atom(format!("{obj}.try_text()")));
        }
        self.need(Need::JavaString(t));
        if self.lazy_text && !self.printing { obj } else { format!("{obj}.java_string()") }
    }

    /// Whether turning a value of the enum of `t` into text may throw an exception that the
    /// program observes: some variant's `toString` throws, or the `hashCode` that `Object`'s
    /// `toString` shows.
    pub(super) fn enum_text_throws(&self, t: usize) -> bool {
        self.enum_variants(t)
            .into_iter()
            .filter_map(|c| hierarchy::to_string_impl(self.class, c).or_else(|| self.hash_index(c)))
            .any(|i| self.facts.throwing.contains(&i))
    }

    /// Whether turning a value of the enum of `t` into text may write a shared object.
    pub(super) fn enum_text_writes(&self, t: usize) -> bool {
        self.enum_variants(t)
            .into_iter()
            .filter_map(|c| hierarchy::to_string_impl(self.class, c))
            .any(|i| self.facts.writers.contains(&i))
    }

    /// The enum of type `t` and the items of it that the code uses. In a round of enum items
    /// alone (`Gen::only_enums`), the items asked for anew, as those find what they ask for: all
    /// of them when the enum itself is.
    pub(super) fn type_enum_items(&mut self, t: usize) {
        if self.facts.closure_types.contains(&t) {
            return self.closure_alias(t);
        }
        let all = self.only_enums.as_ref().is_none_or(|only| only.needs.contains(&Need::Enum(t)));
        let anew = |g: &Self, need: Need| {
            all || g.only_enums.as_ref().is_some_and(|only| only.needs.contains(&need))
        };
        let variants = self.enum_variants(t);
        let name = struct_name(&self.class.types[t].name);
        if all {
            let view = self.view_name(t);
            let shorts: Vec<String> = variants
                .iter()
                .map(|&c| self.variant(t, c).rsplit("::").next().unwrap_or_default().to_string())
                .collect();
            let vis = self.vis();
            self.gap();
            self.line("#[derive(Clone, Copy)]");
            self.line("#[repr(transparent)]");
            self.line(&format!("{vis}struct {name}({vis}java::Thin);"));
            self.gap();
            if variants.is_empty() {
                self.line(&format!("{vis}enum {view} {{}}"));
            } else {
                // Each variant's discriminant is its class number, so that `view()` keeps the
                // number it reads instead of mapping it to another, and its arms differ in nothing
                // else.
                self.line("#[repr(u16)]");
                self.line(&format!("{vis}enum {view}<'a> {{"));
                self.indent += 1;
                for (&c, short) in variants.iter().zip(&shorts) {
                    let payload = self.payload(c);
                    self.line(&format!("{short}(&'a {payload}) = {},", self.class_tag(c)));
                }
                self.indent -= 1;
                self.line("}");
            }
            self.thin_impl(&name, &view, &variants, &shorts);
        }
        let wanted: Vec<Need> = (self.impl_needs.get(&t).into_iter().flatten().copied())
            .filter(|&need| anew(self, need))
            .collect();
        let mark = self.out.len();
        self.gap();
        self.line(&format!("impl {name} {{"));
        self.indent += 1;
        let start = self.out.len();
        for need in &wanted {
            match *need {
                Need::Base(_) => self.base_fn(t, false),
                Need::BaseMut(_) => self.base_fn(t, true),
                Need::Convert(_, to) => self.convert_fn(to),
                Need::Narrow(_, to) => self.narrow_fn(t, to),
                Need::Dispatch(_, group) => self.dispatcher(t, group, None),
                Need::Among(_, group, key) => self.dispatcher(t, group, Some(key)),
                Need::Body(i) => {
                    self.gap();
                    self.method(i);
                }
                _ => {}
            }
        }
        self.indent -= 1;
        if self.out.len() == start {
            self.out.truncate(mark);
        } else {
            self.line("}");
        }
        if all {
            self.identity_impl(t, &variants);
            self.same_object_impl(t, &variants);
            self.monitor_impl(t, &variants);
        }
        if self.wanted.contains(&Need::Display(t)) {
            if anew(self, Need::Display(t)) {
                self.display_impl(t, &variants);
                if self.enum_text_throws(t) {
                    self.try_text_impl(t, &variants);
                }
            }
            if self.wanted.contains(&Need::JavaString(t))
                && (anew(self, Need::Display(t)) || anew(self, Need::JavaString(t)))
            {
                self.java_string_impl(t, &variants);
            }
        }
        if self.wanted.contains(&Need::NullableText(t)) && anew(self, Need::NullableText(t)) {
            self.nullable_text_impl(t, &variants);
        }
        self.class_value_impls(t, &variants, &|g, need| anew(g, need));
        if all {
            self.raised_impl(t);
        }
    }

    /// The arms of a `match` on the class number over the variants, each made by `arm` from the
    /// variant's class, which reads it as `o`; a `match` of the empty view for an enum without
    /// variants.
    fn match_arms(&mut self, variants: &[usize], arm: &dyn Fn(&mut Self, usize) -> (String, bool)) {
        self.match_arms_or(variants, None, arm)
    }

    /// `match_arms`, where an object of a class outside `variants` runs `otherwise`, if given.
    pub(super) fn match_arms_or(
        &mut self,
        variants: &[usize],
        otherwise: Option<String>,
        arm: &dyn Fn(&mut Self, usize) -> (String, bool),
    ) {
        if variants.is_empty() {
            self.line("match self.view() {}");
            return;
        }
        let arms: Vec<(usize, String, bool)> = variants
            .iter()
            .map(|&c| {
                let (text, uses) = arm(self, c);
                (c, text, uses)
            })
            .collect();
        // The same call for every variant needs no `match`.
        if otherwise.is_none() && arms.iter().all(|(_, text, uses)| !uses && *text == arms[0].1) {
            self.line(&arms[0].1);
            return;
        }
        // A match on the class number, whose arm lends the object where it lies, as `view()`
        // does: one jump to the class's code rather than one to the view and one to the code.
        self.line("match self.0.tag() {");
        self.indent += 1;
        for (c, text, uses) in arms {
            let tag = self.class_tag(c);
            if uses {
                let payload = self.payload(c);
                let lend = if self.thin_pointer(c) { "as_pointer" } else { "as_value" };
                // The class number is the one its constructor gives.
                self.line(&format!(
                    "{tag} => {{ let o: &{payload} = unsafe {{ self.0.{lend}() }}; {text} }}"
                ));
            } else {
                self.line(&format!("{tag} => {text},"));
            }
        }
        match otherwise {
            Some(text) => self.line(&format!("_ => {text},")),
            None => self.line("tag => java::bad_tag(tag),"),
        }
        self.indent -= 1;
        self.line("}");
    }

    /// `base()` or `base_mut()`: the fields struct of the type's class in each variant's object.
    fn base_fn(&mut self, t: usize, mutable: bool) {
        let Some(root) = self.fields_root(t) else { return };
        let structure = self.struct_of(root);
        let variants = self.enum_variants(t);
        let immutable = self.plain_enum(t);
        if mutable && immutable {
            return;
        }
        self.gap();
        let vis = self.vis();
        // The borrows of a `java::Locked` are the runtime's.
        let java = if self.facts.thread_shared.contains(&root) { "java::" } else { "" };
        self.line(&match (mutable, immutable) {
            (false, true) => format!("{vis}fn base(&self) -> &{structure} {{"),
            (false, false) => format!("{vis}fn base(&self) -> {java}Ref<'_, {structure}> {{"),
            _ => format!("{vis}fn base_mut(&self) -> {java}RefMut<'_, {structure}> {{"),
        });
        self.indent += 1;
        if immutable && !variants.is_empty() && variants.iter().all(|&c| self.object_variant(c)) {
            // Each variant's struct is `#[repr(C)]` with its `base` first (`type_items`), so the
            // root's fields struct is at the start of every object, which is a `java::Monitored`
            // only around it, fields first: one read, whatever the class. rustc checks the offsets.
            let checks: Vec<String> = variants
                .iter()
                .filter_map(|&c| {
                    let path = hierarchy::base_path(self.facts, self.class, c, root);
                    let path = path.trim_end_matches('.');
                    let object = self.struct_of(c);
                    (!path.is_empty())
                        .then(|| format!("assert!(std::mem::offset_of!({object}, {path}) == 0);"))
                })
                .collect();
            if !checks.is_empty() {
                self.line("const {");
                for check in checks {
                    self.line(&format!("    {check}"));
                }
                self.line("}");
            }
            self.line("// Every variant is an object that starts with the fields struct (above).");
            self.line("unsafe { self.0.as_prefix() }");
            self.indent -= 1;
            self.line("}");
            return;
        }
        self.match_arms(&variants, &|g: &mut Self, c| {
            let path = hierarchy::base_path(g.facts, g.class, c, root);
            let path = path.trim_end_matches('.');
            let text = match (mutable, immutable, path.is_empty()) {
                (false, true, true) => "&**o".to_string(),
                (false, true, false) => format!("&o.{path}"),
                (false, false, true) => "o.borrow()".to_string(),
                (false, false, false) => format!("{java}Ref::map(o.borrow(), |o| &o.{path})"),
                (true, _, true) => "o.borrow_mut()".to_string(),
                (true, _, false) => format!("{java}RefMut::map(o.borrow_mut(), |o| &mut o.{path})"),
            };
            (text, true)
        });
        self.indent -= 1;
        self.line("}");
    }

    /// `to_<to>()`: the same object as a value of the enum of a supertype, which has the same
    /// bits: a class has one number in every enum (`class_tag`).
    fn convert_fn(&mut self, to: usize) {
        self.gap();
        let target = struct_name(&self.class.types[to].name);
        self.line(&format!("{}fn {}(&self) -> {target} {{", self.vis(), self.to_name(to)));
        self.line(&format!("    {target}(self.0)"));
        self.line("}");
    }

    /// `as_<to>()`: the object as a value of type `to` if it is one, else `None`.
    fn narrow_fn(&mut self, t: usize, to: usize) {
        let variants = self.enum_variants(t);
        let ty = self.java_type(to);
        let target = self.ty_name(ty);
        let rep = (self.class.types[to].kind != TypeKind::Enum).then(|| self.enum_for(to)).flatten();
        self.gap();
        self.line(&format!("{}fn {}(&self) -> Option<{target}> {{", self.vis(), self.as_name(to)));
        self.indent += 1;
        let passing = self.passing(t, to);
        if passing.is_empty() {
            self.line("None");
        } else if let Some(e) = rep {
            // A value of another enum has the same bits.
            let target = struct_name(&self.class.types[e].name);
            if passing.len() == variants.len() {
                self.line(&format!("Some({target}(self.0))"));
            } else {
                self.line("match self.0.tag() {");
                self.indent += 1;
                self.line(&format!("{} => Some({target}(self.0)),", self.tags(&passing, &variants)));
                self.line("_ => None,");
                self.indent -= 1;
                self.line("}");
            }
        } else {
            self.line("match self.view() {");
            self.indent += 1;
            for &c in &passing {
                self.line(&format!("{}(o) => Some({}),", self.view_pattern(t, c), self.held(c)));
            }
            if passing.len() < variants.len() {
                self.line("_ => None,");
            }
            self.indent -= 1;
            self.line("}");
        }
        self.indent -= 1;
        self.line("}");
    }

    /// The dispatcher on the enum of `t` for an override group: each variant runs what its class
    /// runs for the method.
    fn dispatcher(&mut self, t: usize, group: usize, among: Option<u64>) {
        let h = &self.class.hierarchy;
        let mut chain = vec![t];
        chain.extend(h.ancestors(t));
        // The signature is that of the group's method declared nearest above the type; the
        // analyses give every method of a group the same one.
        let members = h.group(group);
        let above = |m: &usize| h.is_subtype(t, self.class.methods[*m].class);
        let Some(r) = chain
            .iter()
            .find_map(|&a| members.iter().copied().find(|&m| self.class.methods[m].class == a))
            .or_else(|| members.iter().copied().find(above))
        else {
            return;
        };
        self.enter_method(r);
        let m = &self.class.methods[r];
        // The arms bind the object to `o`, which must not hide a parameter of that name.
        let names: Vec<String> = m.params.iter().map(|p| self.local(&p.name)).collect();
        for p in &m.params {
            if self.local(&p.name) == "o" {
                let fresh = (1..).map(|n| format!("o{n}")).find(|f| !names.contains(f)).expect("a free name");
                self.locals.insert(p.name.clone(), fresh);
            }
        }
        let params = self.typed_params(r, false);
        let ret = self.ret_text(r);
        let args: Vec<String> = m.params.iter().map(|p| self.local(&p.name)).collect();
        let args = args.join(", ");
        let general = self.group_names[&group].clone();
        let mut variants = self.enum_variants(t);
        let name = match among {
            Some(key) => {
                let set = &self.receiver_sets[&key];
                variants.retain(|c| set.contains(c));
                format!("{general}_among_{key:016x}")
            }
            None => general.clone(),
        };
        self.gap();
        if among.is_some() {
            self.line("#[inline]");
        }
        let all: String = params.iter().map(|p| format!(", {p}")).collect();
        self.line(&format!("{}fn {name}(&self{all}){ret} {{", self.vis()));
        self.indent += 1;
        // Every object of the enum runs the call that `fields_call` writes, which an object
        // outside the classes of `among` would not.
        if among.is_none()
            && let Some(call) = self.fields_call(t, r, &variants, &args)
        {
            self.line(&call);
            self.indent -= 1;
            self.line("}");
            return;
        }
        let otherwise = among.map(|_| format!("self.{general}({args})"));
        self.match_arms_or(&variants, otherwise, &|g: &mut Self, c| match g
            .class
            .hierarchy
            .implementation(r, c)
        {
            // A class answers `Type`'s `getTypeName()` and `TypeDescriptor`'s
            // `descriptorString()` itself (see `class_answer`).
            _ if g.runtime_of(c) == Some(Type::JavaClass) && g.class_answer(r).is_some() => {
                (g.class_answer_arm(r), true)
            }
            Some(i) => {
                let (text, uses) = g.arm_call(t, c, i, &args);
                (g.covariant_ret(i, r, text), uses)
            }
            // What runs no method of the program cannot throw, but the group may (a `hashCode`
            // that iterates a map), and then every arm gives a `Result`.
            None if g.facts.throwing.contains(&r) => (format!("Ok({})", g.builtin_arm(r, c)), true),
            None => (g.builtin_arm(r, c), true),
        });
        self.indent -= 1;
        self.line("}");
    }

    /// The call of method `r` on the enum of `t` when every variant runs one body emitted on a
    /// fields struct at or above the one `base()` lends, and `base()` reads that with no `match`
    /// (`base_fn`): `self.base().base.get_name()`, one call whatever the object's class.
    fn fields_call(&mut self, t: usize, r: usize, variants: &[usize], args: &str) -> Option<String> {
        let h = &self.class.hierarchy;
        let i = h.implementation(r, *variants.first()?)?;
        let Home::Fields(d) = self.homes[i] else { return None };
        let root = self.fields_root(t)?;
        let direct = self.plain_enum(t)
            && h.is_subtype(root, d)
            && variants.iter().all(|&c| h.implementation(r, c) == Some(i) && self.object_variant(c));
        if !direct {
            return None;
        }
        self.need(Need::Base(t));
        let path = hierarchy::base_path(self.facts, self.class, root, d);
        let call = format!("self.base().{path}{}({args})", self.fn_names[i]);
        Some(self.covariant_ret(i, r, call))
    }

    /// The arm for a value of class `c` bound to `o` that runs no method of the program for
    /// method `r`: `Object`'s `equals` or `hashCode`, or for an enum's constant a method that
    /// every enum has, which `lower` checks: `name()`, `toString()` or `ordinal()`.
    fn builtin_arm(&mut self, r: usize, c: usize) -> String {
        // An interface may declare `Object`'s `equals` or `hashCode` again (as `Comparator` does).
        if self.class.hierarchy.group_members(r).iter().any(|&g| self.is_object_method(g)) {
            return self.object_arm(r, c);
        }
        if self.is_comparable_method(r) {
            return self.compare_arm(r, c);
        }
        if self.is_number_method(r) {
            return self.number_arm(r, c);
        }
        if self.is_char_sequence_method(r) {
            return self.char_sequence_arm(r, c);
        }
        if self.is_appendable_method(r) {
            return self.appendable_arm(r, c);
        }
        let text = match self.class.methods[r].name.as_str() {
            "ordinal" => return "*o as i32".to_string(),
            "name" => "o.enum_name()",
            _ => "o.to_string()",
        };
        // A method that another class answers with null returns an `Option`.
        if self.facts.nullable_returns.contains(&r) { format!("Some({text})") } else { text.to_string() }
    }

    /// The type of `Object`, when the program uses it.
    pub(super) fn object_index(&self) -> Option<usize> {
        // The first type of that name, found by the index, is the one unless it is not the
        // interface.
        let t = self.class.type_index(OBJECT)?;
        if self.class.types[t].kind == TypeKind::Interface {
            return Some(t);
        }
        self.class.types.iter().position(|d| d.kind == TypeKind::Interface && d.name == OBJECT)
    }

    /// The index of `Object`'s method `name`, when the program uses `Object`.
    fn object_method(&self, name: &str) -> Option<usize> {
        let t = self.object_index()?;
        self.class.methods.iter().position(|m| m.class == t && m.name == name)
    }

    /// Whether method `r` is `Object`'s own abstract `equals` or `hashCode`.
    pub(super) fn is_object_method(&self, r: usize) -> bool {
        self.object_index() == Some(self.class.methods[r].class)
    }

    /// Whether method `r` is `Comparable`'s own abstract `compareTo`.
    fn is_comparable_method(&self, r: usize) -> bool {
        let t = &self.class.types[self.class.methods[r].class];
        t.kind == TypeKind::Interface && t.name == COMPARABLE
    }

    /// Whether method `r` is one of `CharSequence`'s own abstract methods.
    fn is_char_sequence_method(&self, r: usize) -> bool {
        let t = &self.class.types[self.class.methods[r].class];
        t.kind == TypeKind::Interface && t.name == CHAR_SEQUENCE
    }

    /// Whether a value of class `c` has no method for `CharSequence`'s method `r`: it is no
    /// string or builder, and no method of the program implements `r` for it.
    fn missing_sequence_method(&self, r: usize, c: usize) -> bool {
        self.runtime_of(c).is_none() && self.class.hierarchy.implementation(r, c).is_none()
    }

    /// `CharSequence`'s `length()`, `charAt(index)`, `subSequence(start, end)` or `getChars(..)`
    /// (method `r`) for a string or a builder of class `c` bound to `o`. What a string's throws
    /// is thrown for a `java::catching` around the call (see `analysis::exceptions`), as a
    /// builder's is.
    fn char_sequence_arm(&mut self, r: usize, c: usize) -> String {
        let method = &self.class.methods[r];
        let builder = self.runtime_of(c) == Some(Type::StringBuilder);
        if self.missing_sequence_method(r, c) {
            // Each call that may reach it is rejected (`enum_call`).
            return "unreachable!()".to_string();
        }
        if method.name == "subSequence" {
            return self.sub_sequence_arm(r, builder);
        }
        if method.name == "getChars" {
            return self.get_chars_arm(r, builder);
        }
        match (method.name.as_str(), builder) {
            ("length", true) => "o.length()".to_string(),
            ("length", false) => format!("{}(o)", self.runtime("string_length")),
            (_, true) => format!("o.char_at({})", self.local(&method.params[0].name)),
            (_, false) => {
                let index = self.local(&method.params[0].name);
                format!(
                    "{}(o, {index}).unwrap_or_else(|e| {}(e))",
                    self.runtime("try_char_at"),
                    self.runtime("throw_exception")
                )
            }
        }
    }

    /// `getChars(srcBegin, srcEnd, dst, dstBegin)` (method `r`) of a string or a builder bound to
    /// `o`, whose `dst` may be null. What it throws is thrown for a `java::catching` around the
    /// call (see `analysis::exceptions`).
    fn get_chars_arm(&mut self, r: usize, builder: bool) -> String {
        let method = &self.class.methods[r];
        let names: Vec<String> = method.params.iter().map(|p| self.local(&p.name)).collect();
        let [begin, end, dst, dst_begin] = &names[..] else { unreachable!("`getChars` has four parameters") };
        let dst = match self.optional_var(r, method.params[2].ty, &method.params[2].name) {
            true => format!("{dst}.as_ref()"),
            false => format!("Some(&{dst})"),
        };
        if builder {
            format!("o.sequence_get_chars({begin}, {end}, {dst}, {dst_begin})")
        } else {
            format!("{}(o, {begin}, {end}, {dst}, {dst_begin})", self.runtime("sequence_get_chars"))
        }
    }

    /// `subSequence(start, end)` (method `r`) of a string or a builder bound to `o`: its
    /// `substring`, as the string variant of `CharSequence`'s enum.
    fn sub_sequence_arm(&mut self, r: usize, builder: bool) -> String {
        let method = &self.class.methods[r];
        let (start, end) = (self.local(&method.params[0].name), self.local(&method.params[1].name));
        let text = if builder {
            format!("o.substring({start}, {end})")
        } else {
            let thrown = self.runtime("throw_exception");
            format!("{}(o, {start}, {end}).unwrap_or_else(|e| {thrown}(e))", self.runtime("try_substring"))
        };
        // A shared string's `substring` of all of it is the string itself.
        let text = match (super::shared_strings(), builder) {
            (true, true) => format!("{}({text})", self.runtime("new_or_empty")),
            (true, false) => format!("{}(o, {text})", self.runtime("kept")),
            (false, _) => text,
        };
        let char_sequence = method.class;
        let string =
            self.class.type_of(Type::String).filter(|&s| self.enum_variants(char_sequence).contains(&s));
        let Some(string) = string else {
            let message = "`subSequence` of a builder held as a `CharSequence` where no string is one not supported yet";
            self.rejected.push(Error { pos: method.pos, message: message.to_string() });
            return "unreachable!()".to_string();
        };
        let value = format!("{}({text})", self.variant(char_sequence, string));
        if self.facts.nullable_returns.contains(&r) { format!("Some({value})") } else { value }
    }

    /// The method of `CLASS_VALUE_INTERFACES` that method `r` is or overrides, by name, when a
    /// class answers it with its own: `Type`'s `getTypeName()`, where the interface's default
    /// would give the class's `toString()`, `TypeDescriptor`'s abstract `descriptorString()`, and
    /// `GenericDeclaration`'s `getTypeParameters()`, which is rejected.
    fn class_answer(&self, r: usize) -> Option<&'static str> {
        self.class.hierarchy.group(r).into_iter().find_map(|g| {
            let m = &self.class.methods[g];
            match (self.class.types[m.class].name.as_str(), m.name.as_str()) {
                _ if !m.params.is_empty() => None,
                ("java.lang.reflect.Type", "getTypeName") => Some("getTypeName"),
                ("java.lang.invoke.TypeDescriptor", "descriptorString") => Some("descriptorString"),
                ("java.lang.reflect.GenericDeclaration", "getTypeParameters") => Some("getTypeParameters"),
                _ => None,
            }
        })
    }

    /// Method `r` (see `class_answer`) for a class bound to `o`: `Class`'s own, as the JDK makes
    /// it.
    fn class_answer_arm(&mut self, r: usize) -> String {
        if self.class_answer(r) == Some("getTypeParameters") {
            let message = "`getTypeParameters` of a class held as a `GenericDeclaration` not supported yet";
            self.rejected.push(Error { pos: self.class.methods[r].pos, message: message.to_string() });
            return "unreachable!()".to_string();
        }
        let text = match self.class_answer(r) {
            Some("getTypeName") if super::shared_strings() => "o.shared_type_name()".to_string(),
            Some("getTypeName") => "o.get_type_name()".to_string(),
            _ if super::shared_strings() => format!("{}(o.descriptor_string())", self.runtime("JStr::new")),
            _ => "o.descriptor_string()".to_string(),
        };
        let text = if self.facts.nullable_returns.contains(&r) { format!("Some({text})") } else { text };
        if self.facts.throwing.contains(&r) { format!("Ok({text})") } else { text }
    }

    /// Whether method `r` is one of the JDK's `Appendable`'s own `append` methods, which a builder
    /// answers (see `lower::object`).
    fn is_appendable_method(&self, r: usize) -> bool {
        let t = &self.class.types[self.class.methods[r].class];
        t.kind == TypeKind::Interface && t.name == "java.lang.Appendable"
    }

    /// `Appendable`'s `append(csq)`, `append(csq, start, end)` or `append(c)` (method `r`) for a
    /// builder of class `c` bound to `o`: the builder's own, which returns the builder itself. A
    /// null `csq` appends `"null"`, as Java's does.
    fn appendable_arm(&mut self, r: usize, c: usize) -> String {
        let method = &self.class.methods[r];
        let appendable = method.class;
        if self.runtime_of(c) != Some(Type::StringBuilder) {
            return "unreachable!()".to_string();
        }
        let names: Vec<String> = method.params.iter().map(|p| self.local(&p.name)).collect();
        let append = match (&method.params[..], &names[..]) {
            ([p], [x]) if p.ty == Type::Char => format!("o.append_char({x})"),
            ([p, ..], [csq, rest @ ..]) if matches!(p.ty, Type::Class(..)) => {
                if let Some(t) = self.type_of(p.ty) {
                    self.need(Need::JavaString(t));
                }
                let call = |text: &str| match rest {
                    [] => Some(format!("o.append_str({text})")),
                    [start, end] => Some(format!("o.append_range({text}, {start}, {end})")),
                    _ => None,
                };
                let null = format!("{}(\"null\")", self.runtime("wtf8"));
                let (Some(present), Some(absent)) = (call("&s.java_string()"), call(&null)) else {
                    return "unreachable!()".to_string();
                };
                if self.optional_var(r, p.ty, &p.name) {
                    format!("match &{csq} {{ Some(s) => {{ {present}; }} None => {{ {absent}; }} }}")
                } else {
                    call(&format!("&{csq}.java_string()")).unwrap_or_default()
                }
            }
            _ => return "unreachable!()".to_string(),
        };
        let builder = format!("{}(o.clone())", self.variant(appendable, c));
        let value =
            if self.facts.nullable_returns.contains(&r) { format!("Some({builder})") } else { builder };
        format!("{{ {append}; {value} }}")
    }

    /// Whether method `r` is one of `Number`'s own abstract `byteValue()` to `doubleValue()`.
    fn is_number_method(&self, r: usize) -> bool {
        let t = &self.class.types[self.class.methods[r].class];
        t.kind == TypeKind::Interface && t.name == NUMBER
    }

    /// `Number`'s `intValue()` and the others (method `r`) for a box of class `c` bound to `o`:
    /// its value converted as Java's primitive conversions convert it, which Rust's `as` does
    /// (wrapping to narrower integers, saturating from floating point, NaN to 0). Floating point
    /// becomes a `byte` or a `short` through an `int`, as in Java.
    fn number_arm(&self, r: usize, c: usize) -> String {
        let ret = self.class.methods[r].ret;
        let Some(ty) = self.runtime_of(c) else { return "unreachable!()".to_string() };
        let value = if self.faithful_box(ty) { "**o" } else { "*o" };
        let from = ty.unboxed();
        if from == ret {
            return value.to_string();
        }
        if matches!(from, Type::Float | Type::Double) && matches!(ret, Type::Byte | Type::Short) {
            return format!("(({value}) as i32) as {}", rust_type(ret));
        }
        format!("({value}) as {}", rust_type(ret))
    }

    /// `Comparable`'s `compareTo` (method `r`) for a value of class `c` bound to `o`, a string,
    /// a box or an enum's constant, compared with the argument, which must be one of the same
    /// class.
    fn compare_arm(&mut self, r: usize, c: usize) -> String {
        let p = &self.class.methods[r].params[0];
        let other = self.local(&p.name);
        let optional = self.optional_var(r, p.ty, &p.name);
        let place = if self.lends(r) { other } else { format!("&{other}") };
        let comparable = self.class.methods[r].class;
        let compare = match self.runtime_of(c) {
            Some(ty) => {
                let star = if self.faithful_box(ty) { "**" } else { "*" };
                let (a, b) = (format!("{star}o"), format!("{star}b"));
                match ty.unboxed() {
                    Type::String => format!("{}(o, b)", self.runtime("compare_to")),
                    Type::Int | Type::Long => format!("({a}).cmp(&{b}) as i32"),
                    Type::Boolean => format!("{}({a}, {b})", self.runtime("bool_compare")),
                    Type::Double => format!("{}({a}, {b})", self.runtime("double_compare")),
                    Type::Float => {
                        format!("{}(f64::from({a}), f64::from({b}))", self.runtime("double_compare"))
                    }
                    _ => format!("i32::from({a}) - i32::from({b})"),
                }
            }
            // An enum's constants compare by their ordinals.
            None => "*o as i32 - *b as i32".to_string(),
        };
        let target = self.cast_class(c);
        let failed = |g: &mut Self, from: String| format!("{}({from}, {target})", g.runtime("cast_failed"));
        let e = match self.rep(comparable) {
            hierarchy::Rep::Enum(e) => e,
            // Every `Comparable` is of the one class `x`.
            hierarchy::Rep::Object(x) => {
                let from = self.cast_class(x);
                return failed(self, from);
            }
        };
        let variant = self.view_pattern(e, c);
        let name = struct_name(&self.class.types[e].name);
        // The argument is a parameter, read again where it is not of the class.
        let viewed = if optional {
            format!("{}.as_ref().map({name}::view)", operand(&place))
        } else {
            format!("{}.view()", operand(&place))
        };
        let pattern = if optional { format!("Some({variant}(b))") } else { format!("{variant}(b)") };
        if !optional && self.enum_variants(e).len() == 1 {
            return format!("{{ let {pattern} = {viewed}; {compare} }}");
        }
        // Any other object fails the cast to `c` that `compareTo` makes; null fails first.
        self.need(Need::CastSource(e));
        let from = format!("{}::cast_class(&b)", self.runtime("CastSource"));
        let fail = failed(self, from);
        if optional {
            // A null argument fails inside the class's own `compareTo`, with that one's message: a
            // string's or a box's as a `Result` that the `?` of `compare_to` passes on, where the
            // group gives a `Result`. Otherwise (an enum's, or a group of only `i32`s) it is
            // thrown, which a `java::catching` around the call turns into the exception a
            // `catch` sees. Any other class still fails the cast.
            let other = match self.runtime_of(c) {
                Some(ty) if self.facts.throwing.contains(&r) => {
                    let message = compare_null_message(ty.unboxed());
                    let checked =
                        format!("{}(b.as_ref(), {message:?})?", self.runtime("try_non_null_message"));
                    format!("{{ let b = {checked}; {fail} }}")
                }
                _ => {
                    let message = match self.runtime_of(c) {
                        Some(ty) => compare_null_message(ty.unboxed()),
                        None => ENUM_COMPARE_NULL.to_string(),
                    };
                    let throw =
                        format!("{}(\"java.lang.NullPointerException\", {message:?})", self.runtime("throw"));
                    format!("{{ let Some(b) = b.as_ref() else {{ {throw} }}; {fail} }}")
                }
            };
            return format!(
                "match {viewed} {{ {pattern} => {compare}, _ => {{ let b = {place}; {other} }} }}"
            );
        }
        format!("match {viewed} {{ {pattern} => {compare}, _ => {{ let b = {place}; {fail} }} }}")
    }

    /// `Object`'s `equals` or `hashCode` (method `r`) for a value of class `c` bound to `o`
    /// whose class overrides neither: identity for an object, the value's own equality and hash
    /// for an enum's constant, a string or a box.
    fn object_arm(&mut self, r: usize, c: usize) -> String {
        let m = &self.class.methods[r];
        if m.name == "hashCode" {
            return self.value_hash(c, "o");
        }
        let p = &m.params[0];
        let other = self.local(&p.name);
        let optional = self.optional_var(r, p.ty, &p.name);
        let place = if self.lends(r) { other } else { format!("&{other}") };
        let same = match self.runtime_of(c).map(Type::unboxed) {
            Some(Type::Double | Type::Float) => "o.to_bits() == b.to_bits()",
            Some(Type::Array(_)) => "Arc::ptr_eq(o, b)",
            Some(Type::StringBuilder) => "o.same(b)",
            // A box whose identity is kept (`java::Boxed`) compares the values it holds.
            _ if self.runtime_of(c).is_some_and(|ty| self.faithful_box(ty)) => "**o == **b",
            _ if self.holds_value(c) => "o == b",
            _ => "Arc::ptr_eq(o, b)",
        };
        self.equals_test(c, &place, optional, same)
    }

    /// Whether `place`, a reference to an `Object` (an `Option` of one when `optional`), is a
    /// value of class `c` that binds `b` and passes `same`.
    fn equals_test(&mut self, c: usize, place: &str, optional: bool, same: &str) -> String {
        let Some(object) = self.object_index() else { return "false".to_string() };
        let Some(e) = self.enum_for(object) else {
            // Every `Object` is an object of one class.
            if self.holds_value(c) || self.rep(object) != hierarchy::Rep::Object(c) {
                return "false".to_string();
            }
            return if optional {
                format!("{place}.as_ref().is_some_and(|b| {same})")
            } else {
                format!("{{ let b = {place}; {same} }}")
            };
        };
        if !self.enum_variants(e).contains(&c) {
            return "false".to_string();
        }
        let variant = self.view_pattern(e, c);
        if optional {
            format!("{}.as_ref().is_some_and(|b| matches!(b.view(), {variant}(b) if {same}))", operand(place))
        } else {
            format!("matches!({}.view(), {variant}(b) if {same})", operand(place))
        }
    }

    /// The address of the object of class `c` that `handle`, a `&Rc<..>`, refers to, which is
    /// its identity: that of the class's struct, also where `this` is a reference to it.
    pub(super) fn address(&self, c: usize, handle: &str) -> String {
        if self.facts.owned.contains(&self.class.types[c].name) {
            format!("std::ptr::from_ref({handle})")
        } else if self.facts.immutable.contains(&self.class.types[c].name)
            || self.celled(&self.class.types[c].name)
        {
            format!("{}::as_ptr({handle})", self.pointer(c))
        } else {
            // A method call borrows the handle itself; `&x.as_ptr()` would be a reference to the pointer.
            format!("{}.as_ptr()", handle.strip_prefix('&').unwrap_or(handle))
        }
    }

    /// The address of the object of class `c` that `receiver` is (see `address`).
    pub(super) fn receiver_address(&mut self, receiver: &'a Expr, c: usize) -> String {
        let id = receiver.id;
        let receiver = self.peel(receiver);
        match receiver.kind {
            ExprKind::This if self.this_form == ThisForm::Local => "std::ptr::from_ref(&this)".to_string(),
            // A celled object's `this` is its handle, as a shared object's is.
            ExprKind::This
                if self.this_form == ThisForm::SelfRef
                    || self.is_plain(receiver) && !self.celled_object(receiver) =>
            {
                format!("std::ptr::from_ref({})", self.this_name())
            }
            _ if self.nullable(receiver) => {
                let option = format!("{}.as_ref()", self.object_place(receiver));
                let handle = self.dereference(&[id, receiver.id], &option);
                self.address(c, &handle)
            }
            _ => {
                let handle = self.object_ref(receiver);
                self.address(c, &handle)
            }
        }
    }

    /// Whether the first parameter of method `r` is borrowed, so that it is a reference already.
    fn lends(&self, r: usize) -> bool {
        self.facts.borrowed_objects.contains(&(r, 0))
            || self.facts.borrowed_optional_objects.contains(&(r, 0))
    }

    /// A call of `Object`'s `equals` or `hashCode` (method `r`) on an object of a class that
    /// overrides neither and that no enum represents: its identity.
    pub(super) fn identity_call(&mut self, receiver: &'a Expr, r: usize, args: &'a [Expr]) -> Code {
        let c = self.struct_class(receiver).expect("an object has a class type");
        let address = self.receiver_address(receiver, c);
        if self.class.methods[r].name == "hashCode" {
            return Code::atom(format!("{}({address})", self.runtime("address_hash")));
        }
        let arg = self.call_args(r, args).remove(0);
        let p = &self.class.methods[r].params[0];
        let optional = self.optional_var(r, p.ty, &p.name);
        let place = if self.lends(r) { arg } else { format!("&{arg}") };
        // The object is found before the argument is evaluated, as Java does.
        let temp = self.temp("address");
        let same = format!("std::ptr::addr_eq({temp}, {})", self.address(c, "b"));
        let test = self.equals_test(c, &place, optional, &same);
        // An argument that cannot be the object is still evaluated.
        let test = if test == "false" && has_effects(&args[0]) {
            format!("{{ let _ = {place}; false }}")
        } else {
            test
        };
        Code::atom(format!("{{ let {temp} = {address}; {test} }}"))
    }

    /// `Object.toString()` on a value: the `Display` of its enum.
    pub(super) fn object_to_string(&mut self, receiver: &'a Expr) -> Code {
        let Some(t) = self.enum_of(receiver) else { return self.object_text(receiver) };
        self.need(Need::Display(t));
        let obj = self.object(receiver);
        if self.enum_text_throws(t) {
            return Code::atom(self.passed_on(Code::atom(format!("{obj}.try_text()"))));
        }
        self.need(Need::JavaString(t));
        Code::atom(format!("{obj}.java_string()"))
    }

    /// The hash code of a value of class `c`, reached through `handle`, a reference to what the
    /// variant holds, whose class does not override `hashCode`.
    pub(super) fn value_hash(&mut self, c: usize, handle: &str) -> String {
        if self.class.shared_of(c).is_some() {
            return format!("{}({handle})", self.runtime("identity_hash"));
        }
        let Some(ty) = self.runtime_of(c) else {
            if self.holds_value(c) {
                // An enum's constant hashes by its identity in Java, which nothing can depend on.
                return format!("*{handle} as i32");
            }
            return format!("{}({})", self.runtime("address_hash"), self.address(c, handle));
        };
        // A class's hash is its identity's in Java: there is one class object for each class. A
        // charset's is its name's.
        if matches!(ty, Type::JavaClass | Type::Charset) {
            return format!("{handle}.hash_code()");
        }
        let value = if self.faithful_box(ty) { format!("**{handle}") } else { format!("*{handle}") };
        let function = match ty.unboxed() {
            Type::String => return format!("{}({handle})", self.runtime("string_hash")),
            Type::Int => return value,
            Type::Long => "long_hash",
            Type::Double => "double_hash",
            Type::Float => "float_hash",
            Type::Boolean => "bool_hash",
            _ => return format!("{value} as i32"),
        };
        format!("{}({value})", self.runtime(function))
    }

    /// The text that `Object.toString` makes for an object of class `c` bound to `o` in an arm of
    /// the enum of `t`: the class's binary name and its hash code in hexadecimal.
    fn default_arm_text(&mut self, t: usize, c: usize) -> String {
        let hash = match self.hash_index(c) {
            Some(i) => {
                let call = self.arm_call(t, c, i, "").0;
                self.shown_hash(i, call)
            }
            None => self.value_hash(c, "o"),
        };
        self.default_text(c, hash)
    }

    /// The call `call` of `hashCode` method `i` for the text that `Object.toString` makes: what it
    /// throws ends the program, as what a `toString` throws does where an object's text is made
    /// (`analysis::exceptions`).
    pub(super) fn shown_hash(&mut self, i: usize, call: String) -> String {
        if !self.facts.throwing.contains(&i) {
            return call;
        }
        format!("{call}.unwrap_or_else({})", self.uncaught("uncaught"))
    }

    /// The text that `Object.toString` makes for an object of class `c` whose hash code is `hash`.
    pub(super) fn default_text(&mut self, c: usize, hash: String) -> String {
        format!("{}({}, {hash})", self.runtime("default_text"), rust_string(&self.binary_name(c)))
    }

    /// The `hashCode` of the program that an object of class `c` runs, if it has one.
    pub(super) fn hash_index(&self, c: usize) -> Option<usize> {
        let hash_code = *self.object_hash_code.get_or_init(|| self.object_method("hashCode"));
        hash_code.and_then(|h| self.class.hierarchy.implementation(h, c))
    }

    /// The call that the dispatcher of the enum of `t` makes for an object of class `c` bound to
    /// `o`, running method `i`; and whether it uses `o`.
    fn arm_call(&mut self, t: usize, c: usize, i: usize, args: &str) -> (String, bool) {
        let name = self.fn_names[i].clone();
        let class_name = &self.class.types[c].name;
        let immutable = self.facts.immutable.contains(class_name) || self.celled(class_name);
        let borrow = match () {
            _ if immutable || self.holds_value(c) => "",
            _ if self.facts.mutating.contains(&i) => ".borrow_mut()",
            _ => ".borrow()",
        };
        match self.homes[i] {
            // A Java enum's constant is a `Copy` value whose methods take it by value: through
            // the reference, `to_string(locale, messages)` would be `ToString`'s.
            Home::Class(_) if self.class.types[c].kind == TypeKind::Enum => {
                (format!("(*o).{name}({args})"), true)
            }
            Home::Class(_) if self.facts.self_methods.contains(&i) || self.holds_value(c) => {
                (format!("o{borrow}.{name}({args})"), true)
            }
            Home::Class(_) => {
                let all = if args.is_empty() { "o".to_string() } else { format!("o, {args}") };
                (format!("{}({all})", self.qualified_fn(i)), true)
            }
            Home::Fields(d) => {
                let path = hierarchy::base_path(self.facts, self.class, c, d);
                (format!("o{borrow}.{path}{name}({args})"), true)
            }
            Home::Enum(d) => {
                self.need(Need::Body(i));
                if d == t {
                    (format!("self.{name}({args})"), false)
                } else if !self.class.hierarchy.is_subtype(t, d) {
                    // A body inherited from a type below `t`: the object as a value of that type.
                    (format!("{}({}).{name}({args})", self.variant(d, c), self.held(c)), true)
                } else {
                    self.need(Need::Convert(t, d));
                    (format!("self.{}().{name}({args})", self.to_name(d)), false)
                }
            }
            Home::Free | Home::Nowhere => {
                unreachable!("a concrete class runs an instance method with a body")
            }
        }
    }

    /// Identity: two values are equal when they are the same object. Two values with the same
    /// bits (class number and address, or the value held in the low bits) are always equal; a
    /// variant compared by identity is equal only then, so only the variants compared by value
    /// (`same_variant`) look further, through the view.
    fn identity_impl(&mut self, t: usize, variants: &[usize]) {
        let name = struct_name(&self.class.types[t].name);
        self.gap();
        self.line(&format!("impl PartialEq for {name} {{"));
        self.indent += 1;
        self.line(&format!("fn eq(&self, other: &{name}) -> bool {{"));
        self.indent += 1;
        let by_value: Vec<(usize, String)> = variants
            .iter()
            .map(|&c| (c, self.same_variant(c)))
            .filter(|(_, same)| same != IDENTITY)
            .collect();
        if variants.is_empty() {
            self.line("match self.view() {}");
        } else if by_value.is_empty() {
            self.line("self.0 == other.0");
        } else {
            let tags: Vec<String> = by_value.iter().map(|&(c, _)| self.class_tag(c).to_string()).collect();
            self.line(&format!(
                "self.0 == other.0 || self.0.tag() == other.0.tag() && matches!(self.0.tag(), {}) && match (self.view(), other.view()) {{",
                tags.join(" | ")
            ));
            self.indent += 1;
            for (c, same) in by_value {
                let v = self.view_pattern(t, c);
                self.line(&format!("({v}(a), {v}(b)) => {same},"));
            }
            self.line("_ => false,");
            self.indent -= 1;
            self.line("}");
        }
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
    }

    /// Whether `a` and `b`, the objects of two values of class `c`'s variant, are equal for the
    /// enum's `PartialEq`.
    fn same_variant(&mut self, c: usize) -> String {
        // An array is a shared handle, the same array only when it is the same handle; in a
        // program with threads, an `Arc`, as a shared object is.
        let array = self.class.array_of(c).is_some();
        // A box whose identity is kept is the same box only when it is the same object.
        let faithful = self.runtime_of(c).is_some_and(|ty| self.faithful_box(ty));
        // A floating box held as a value is compared as `equals` compares it, so that where the
        // translation compares values for Java's identity (a shortcut before `equals`), `-0.0`
        // differs from `0.0` and NaN equals NaN.
        let floating =
            matches!(self.runtime_of(c), Some(Type::Boxed(b)) if matches!(*b, Type::Double | Type::Float));
        match () {
            _ if self.runtime_of(c) == Some(Type::StringBuilder) => "a.same(b)".to_string(),
            _ if faithful => format!("{}(Some(a), Some(b))", self.runtime("same_optional")),
            _ if floating && self.holds_value(c) => {
                format!("{}(f64::from(*a), f64::from(*b))", self.runtime("double_equals"))
            }
            _ if self.holds_value(c) && !array => "a == b".to_string(),
            _ => IDENTITY.to_string(),
        }
    }

    /// `java::SameObject` for the enum of `Object` (type `t`), where the program compares values
    /// that may both be strings or boxes (`Facts::identity_checks`): Java's identity, which a
    /// string or a box that is not kept as an object (`faithful_box`) has only as far as unequal
    /// values are different objects, but for `Boolean` and `Byte`, whose equal values are one.
    fn same_object_impl(&mut self, t: usize, variants: &[usize]) {
        if self.facts.identity_checks.is_empty() || Some(t) != self.object_index() {
            return;
        }
        let name = struct_name(&self.class.types[t].name);
        let trait_name = self.runtime("SameObject");
        self.gap();
        self.line(&format!("impl {trait_name} for {name} {{"));
        self.indent += 1;
        self.line(&format!("fn same_object(&self, other: &{name}) -> bool {{"));
        self.indent += 1;
        if variants.is_empty() {
            self.line("match self.view() {}");
        } else {
            self.line("match (self.view(), other.view()) {");
            self.indent += 1;
            for &c in variants {
                let v = self.view_pattern(t, c);
                let mut same = self.same_variant(c);
                // A `Boolean` or a `Byte` is one of `valueOf`'s cached objects, the same object
                // exactly when the values are equal (see `analysis::boxing`). Shared strings are
                // compared as objects.
                let unkept = match self.runtime_of(c) {
                    Some(Type::String) if super::shared_strings() => {
                        same = format!("{}(a, b)", self.runtime("same_str"));
                        false
                    }
                    Some(Type::String) => true,
                    Some(ty @ Type::Boxed(b)) => {
                        !matches!(*b, Type::Boolean | Type::Byte) && !self.faithful_box(ty)
                    }
                    _ => false,
                };
                if unkept {
                    let end = self.runtime("unkept_identity");
                    self.line(&format!("({v}(a), {v}(b)) => {same} && {end}(),"));
                } else {
                    self.line(&format!("({v}(a), {v}(b)) => {same},"));
                }
            }
            if variants.len() > 1 {
                self.line("_ => false,");
            }
            self.indent -= 1;
            self.line("}");
        }
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
    }

    /// The monitor of an object of the type, where the program keeps the monitors of its objects
    /// (`synchronized (node)` on a `ConcurrentHashMap` bin): the monitor of the variant's handle.
    /// A variant that is not such a handle (a value, such as a string or a box held as an
    /// `Object`, or an immutable object shared as it is) has none, and ends the program
    /// (`java::unkept_monitor`): `synchronized (LOCK)` on an `Object` that is a plain object
    /// keeps its monitor although the type's other values have none.
    fn monitor_impl(&mut self, t: usize, variants: &[usize]) {
        if !self.facts.kept_monitor_of(t) {
            return;
        }
        let (has, of) = (self.runtime("HasMonitor"), self.runtime("MonitorOf"));
        let name = struct_name(&self.class.types[t].name);
        // A type with no objects (`synchronized (lock)` on a `FileLock` of a channel that no
        // code makes): there is no monitor to give.
        if variants.is_empty() {
            self.gap();
            self.line(&format!("impl {has} for {name} {{"));
            self.indent += 1;
            self.line(&format!("fn monitor(&self) -> {of}<'_> {{"));
            self.indent += 1;
            self.line("match self.view() {}");
            self.indent -= 1;
            self.line("}");
            self.indent -= 1;
            self.line("}");
            return;
        }
        // A builder's handle has its cell's monitor (`java::Builder`'s `HasMonitor`).
        let handles: Vec<bool> = variants
            .iter()
            .map(|&c| {
                let payload = self.payload(c);
                payload.starts_with("Arc<RefCell<")
                    || payload.starts_with("Arc<java::Locked<")
                    || payload.starts_with("Arc<java::Monitored<")
                    || matches!(payload.as_str(), "java::StringBuilder" | "java::SharedStringBuilder")
            })
            .collect();
        if !handles.contains(&true) {
            return;
        }
        self.gap();
        self.line(&format!("impl {has} for {name} {{"));
        self.indent += 1;
        self.line(&format!("fn monitor(&self) -> {of}<'_> {{"));
        self.indent += 1;
        self.line("match self.view() {");
        self.indent += 1;
        let unkept = self.runtime("unkept_monitor");
        for (&c, handle) in variants.iter().zip(handles) {
            let v = self.view_pattern(t, c);
            if handle {
                self.line(&format!("{v}(o) => {has}::monitor(o),"));
            } else {
                self.line(&format!("{v}(_) => {unkept}(),"));
            }
        }
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
    }

    /// `Display`: each object shows as its class's `toString` makes it.
    fn display_impl(&mut self, t: usize, variants: &[usize]) {
        let name = struct_name(&self.class.types[t].name);
        self.gap();
        self.line(&format!("impl fmt::Display for {name} {{"));
        self.indent += 1;
        self.line("fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {");
        self.indent += 1;
        self.match_arms(variants, &|g: &mut Self, c| {
            match g.runtime_of(c) {
                Some(Type::String) => return ("fmt::Display::fmt(o, f)".to_string(), true),
                // A builder shows its text, as its `toString` makes it.
                Some(Type::StringBuilder) => return ("fmt::Display::fmt(o, f)".to_string(), true),
                // An array shows as `[I@` and its identity hash.
                Some(Type::Array(_)) => {
                    let text = g.default_arm_text(t, c);
                    return (format!("fmt::Display::fmt(&{text}, f)"), true);
                }
                Some(ty) => {
                    let value = if g.faithful_box(ty) { "**o" } else { "*o" };
                    let shown = g.shown(ty.unboxed(), value.to_string());
                    return (format!("fmt::Display::fmt(&{shown}, f)"), true);
                }
                None => {}
            }
            if g.holds_value(c) {
                return ("fmt::Display::fmt(o, f)".to_string(), true);
            }
            match hierarchy::to_string_impl(g.class, c) {
                Some(i) => g.display_arm(t, c, i),
                None => {
                    let text = g.default_arm_text(t, c);
                    (format!("fmt::Display::fmt(&{text}, f)"), true)
                }
            }
        });
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
    }

    /// `try_text()`: the text of a value whose class's `toString`, or the `hashCode` that
    /// `Object`'s `toString` shows, may throw, or its exception; the other classes' text is as
    /// `java_string` makes it.
    fn try_text_impl(&mut self, t: usize, variants: &[usize]) {
        let name = struct_name(&self.class.types[t].name);
        let throwable = self.throwable_type();
        self.gap();
        self.line(&format!("impl {name} {{"));
        self.indent += 1;
        let text = self.string_type();
        self.line(&format!("pub(crate) fn try_text(&self) -> Result<{text}, {throwable}> {{"));
        self.indent += 1;
        self.match_arms(variants, &|g: &mut Self, c| match hierarchy::to_string_impl(g.class, c) {
            Some(i) if g.facts.throwing.contains(&i) => {
                let (call, uses) = g.arm_call(t, c, i, "");
                if g.facts.nullable_returns.contains(&i) {
                    let null = g.string_literal("\"null\"");
                    (format!("{call}.map(|text| text.unwrap_or_else(|| {null}))"), uses)
                } else {
                    (call, uses)
                }
            }
            None if g.hash_index(c).is_some_and(|i| g.facts.throwing.contains(&i)) => {
                let i = g.hash_index(c).expect("a hashCode that throws");
                let (call, uses) = g.arm_call(t, c, i, "");
                let text = g.default_text(c, "hash".to_string());
                let text = if super::shared_strings() {
                    format!("{}({text})", g.runtime("JStr::new"))
                } else {
                    text
                };
                (format!("{call}.map(|hash| {text})"), uses)
            }
            _ => {
                let (text, uses) = g.java_string_arm(t, c);
                (format!("Ok({text})"), uses)
            }
        });
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
    }

    /// `java_string()`: the string that `toString` returns for an object of the enum of `t`
    /// whose `toString` does not throw.
    fn java_string_impl(&mut self, t: usize, variants: &[usize]) {
        let name = struct_name(&self.class.types[t].name);
        let text = self.string_type();
        self.gap();
        self.line(&format!("impl {name} {{"));
        self.indent += 1;
        self.line(&format!("pub(crate) fn java_string(&self) -> {text} {{"));
        self.indent += 1;
        self.match_arms(variants, &|g: &mut Self, c| g.java_string_arm(t, c));
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
        // In a concatenation, the string as it is.
        let (piece, jstring) = (self.runtime("Piece"), self.runtime("JString"));
        self.gap();
        self.line(&format!("impl {piece} for {name} {{"));
        self.indent += 1;
        self.line(&format!("fn push_to(&self, out: &mut {jstring}) {{"));
        self.indent += 1;
        self.line("out.push_wtf8(&self.java_string());");
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
    }

    /// The Rust type of a Java string: a `java::JStr` where the program's strings are shared.
    pub(super) fn string_type(&mut self) -> String {
        if super::shared_strings() { self.runtime("JStr") } else { self.runtime("JString") }
    }

    /// The string that `toString` returns for an object of class `c`, a variant of the enum of `t`
    /// bound to `o`: the string itself, what the program's `toString` returns ("null" for null;
    /// what it throws ends the program), a builder's text, or the text of its `Display`; where
    /// the program's strings are shared, an enum constant's name is its interned string, and other
    /// text a new string.
    fn java_string_arm(&mut self, t: usize, c: usize) -> (String, bool) {
        let shared = super::shared_strings();
        let made = |g: &mut Self, text: &str| {
            if shared {
                format!("{}({text})", g.runtime("JStr::new"))
            } else {
                format!("{}::from({text})", g.runtime("JString"))
            }
        };
        match self.runtime_of(c) {
            Some(Type::String) => return ("o.clone()".to_string(), true),
            Some(Type::StringBuilder) if !shared => return ("o.text()".to_string(), true),
            Some(Type::StringBuilder) => return (made(self, "o.text()"), true),
            Some(_) => return (made(self, "self.to_string()"), false),
            None => {}
        }
        match hierarchy::to_string_impl(self.class, c) {
            Some(i) => {
                let (mut call, uses) = self.arm_call(t, c, i, "");
                // What a `toString` throws here ends the program, as where the runtime makes the
                // text (see `display_arm`); `try_text` passes it on.
                if self.facts.throwing.contains(&i) {
                    if let Some(t) =
                        self.class.type_index("java.lang.Throwable").and_then(|t| self.enum_for(t))
                    {
                        self.need(Need::Display(t));
                    }
                    call = format!("{call}.unwrap_or_else({})", self.runtime("uncaught"));
                }
                if self.facts.nullable_returns.contains(&i) {
                    let null = self.string_literal("\"null\"");
                    return (format!("{call}.unwrap_or_else(|| {null})"), uses);
                }
                (call, uses)
            }
            None if shared && self.class.types[c].kind == TypeKind::Enum => {
                (format!("{}(self.to_string())", self.runtime("interned_text")), false)
            }
            None => (made(self, "self.to_string()"), false),
        }
    }

    /// `nullable_text()`: what `toString` returns for an object of the enum of `t`, its null as
    /// `None`, or its exception; where an implementation may return null, the `Display` shows "null".
    /// In a program without exceptions (no `Throwable`) it is the `Option` alone.
    fn nullable_text_impl(&mut self, t: usize, variants: &[usize]) {
        let name = struct_name(&self.class.types[t].name);
        let fallible = self.has_throwable();
        let text = self.string_type();
        let ret = if fallible {
            format!("Result<Option<{text}>, {}>", self.throwable_type())
        } else {
            format!("Option<{text}>")
        };
        self.gap();
        self.line(&format!("impl {name} {{"));
        self.indent += 1;
        self.line(&format!("pub(crate) fn nullable_text(&self) -> {ret} {{"));
        self.indent += 1;
        let throws = self.enum_text_throws(t);
        let ok = |text: String| if fallible { format!("Ok({text})") } else { text };
        self.match_arms(variants, &|g: &mut Self, c| match hierarchy::to_string_impl(g.class, c) {
            Some(i) if g.facts.throwing.contains(&i) || g.facts.nullable_returns.contains(&i) => {
                let (call, uses) = g.arm_call(t, c, i, "");
                let call = match (g.facts.throwing.contains(&i), g.facts.nullable_returns.contains(&i)) {
                    (true, true) => call,
                    (true, false) => format!("{call}.map(Some)"),
                    (false, _) => ok(call),
                };
                (call, uses)
            }
            _ if throws => ("self.try_text().map(Some)".to_string(), false),
            _ => {
                let (text, uses) = g.java_string_arm(t, c);
                (ok(format!("Some({text})")), uses)
            }
        });
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
    }

    /// The methods of the enum of `t` that answer for the object's class, each written once here
    /// rather than as a `match` over every variant wherever it is used: `Object`'s enum has a
    /// variant for every class that the program creates, and javac's has thousands.
    fn class_value_impls(&mut self, t: usize, variants: &[usize], anew: &dyn Fn(&Self, Need) -> bool) {
        if self.wanted.contains(&Need::CopyAcross(t)) && anew(self, Need::CopyAcross(t)) {
            self.copy_across_impl(t, variants);
        }
        let wanted = |need: Need| self.wanted.contains(&need) && anew(self, need);
        let methods = [
            (Need::RuntimeClass(t), "runtime_class", "java::Class"),
            (Need::RuntimeClassName(t), "runtime_class_name", "&'static str"),
            (Need::RuntimeTypeNames(t), "runtime_type_names", "&'static [&'static str]"),
            (Need::IdentityHash(t), "object_identity_hash", "i32"),
            (Need::CopyKind(t), "array_copy_kind", "java::CopyKind"),
        ];
        let methods: Vec<_> = methods.into_iter().filter(|&(need, ..)| wanted(need)).collect();
        let name = struct_name(&self.class.types[t].name);
        if wanted(Need::CastSource(t)) {
            self.gap();
            let (source, class) = (self.runtime("CastSource"), self.runtime("CastClass"));
            self.line(&format!("impl {source} for {name} {{"));
            self.indent += 1;
            self.line(&format!("fn cast_class(&self) -> {class} {{"));
            self.indent += 1;
            self.match_arms(variants, &|g: &mut Self, c| (g.cast_class(c), false));
            self.indent -= 1;
            self.line("}");
            self.indent -= 1;
            self.line("}");
        }
        if methods.is_empty() {
            return;
        }
        self.gap();
        self.line(&format!("impl {name} {{"));
        self.indent += 1;
        for (need, method, ret) in methods {
            self.line(&format!("pub(crate) fn {method}(&self) -> {ret} {{"));
            self.indent += 1;
            self.match_arms(variants, &|g: &mut Self, c| match need {
                // An `Object[]` that may be a covariant array, which carries its class.
                Need::RuntimeClass(_) if g.class.array_of(c).is_some_and(|ty| g.carries_class(ty)) => {
                    (format!("{}(o, || {})", g.runtime("array_class"), g.decl_class(c)), true)
                }
                Need::RuntimeClass(_) => (g.decl_class(c), false),
                Need::RuntimeClassName(_) => (rust_string(&g.java_class_name(c, false)), false),
                Need::RuntimeTypeNames(_) => (g.names_list(c), false),
                Need::CopyKind(_) => (g.copy_kind(c), false),
                _ => (g.value_hash(c, "o"), true),
            });
            self.indent -= 1;
            self.line("}");
        }
        self.indent -= 1;
        self.line("}");
    }

    /// How the `Display` of the enum of `t` shows an object of class `c` bound to `o`, whose
    /// `toString` is method `i`; and whether it uses `o`.
    fn display_arm(&mut self, t: usize, c: usize, i: usize) -> (String, bool) {
        let (mut text, uses) = self.arm_call(t, c, i, "");
        // Where the runtime makes the text, what a `toString` throws ends the program (see
        // `analysis::exceptions`); the generator's own text calls it (`try_text`).
        if self.facts.throwing.contains(&i) {
            if let Some(t) = self.class.type_index("java.lang.Throwable").and_then(|t| self.enum_for(t)) {
                self.need(Need::Display(t));
            }
            text = format!("{text}.unwrap_or_else({})", self.runtime("uncaught"));
        }
        // A `toString` that may return null shows "null", as `String.valueOf` does.
        if self.facts.nullable_returns.contains(&i) {
            return (
                format!("fmt::Display::fmt({text}.as_deref().unwrap_or(java::wtf8(\"null\")), f)"),
                uses,
            );
        }
        (format!("fmt::Display::fmt(&{text}, f)"), uses)
    }
}

/// The enum items that a pass of the generator asked for, which the next pass writes.
pub(super) type Needs = BTreeSet<Need>;

/// The value of an object of class `c` where the program creates none, so the code never runs.
pub(super) fn absent(g: &Gen, c: usize) -> String {
    format!("unreachable!(\"no object of {} is created\")", g.class.types[c].name)
}

/// `name` with its first letter in upper case: `Int` for `int`.
fn capitalized(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

/// The primitive types whose boxes `Array.set` widens to an element of type `ty`, `ty` first.
fn widening_sources(ty: Type) -> &'static [Type] {
    match ty {
        Type::Short => &[Type::Short, Type::Byte],
        Type::Int => &[Type::Int, Type::Short, Type::Byte, Type::Char],
        Type::Long => &[Type::Long, Type::Int, Type::Short, Type::Byte, Type::Char],
        Type::Float => &[Type::Float, Type::Long, Type::Int, Type::Short, Type::Byte, Type::Char],
        Type::Double => {
            &[Type::Double, Type::Float, Type::Long, Type::Int, Type::Short, Type::Byte, Type::Char]
        }
        Type::Byte => &[Type::Byte],
        Type::Char => &[Type::Char],
        _ => &[Type::Boolean],
    }
}
