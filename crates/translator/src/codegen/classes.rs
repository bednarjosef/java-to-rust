//! Comparing classes (see `lower::classes`): an object's class with a class literal is a
//! `matches!` of its enum's variant, and two objects' classes are their names.

use super::*;

impl<'a> Gen<'a> {
    /// `SameClass { same }(a, b)`: whether the classes `a` and `b` are the same (`!same`: are
    /// not). Each is the class of an object, `ClassName(o)`, or a class literal,
    /// `ClassLiteralName`.
    pub(super) fn same_class(&mut self, same: bool, args: &'a [Expr]) -> Code {
        match (literal_class(&args[0]), literal_class(&args[1])) {
            (Some(a), Some(b)) => Code::atom(((a == b) == same).to_string()),
            (Some(c), None) => self.has_class(object_of(&args[1]), c, same),
            (None, Some(c)) => self.has_class(object_of(&args[0]), c, same),
            (None, None) => {
                let (a, b) = (object_of(&args[0]), object_of(&args[1]));
                if let Some(c) = self.known_class(a) {
                    return self.has_class(b, c, same);
                }
                if let Some(c) = self.known_class(b) {
                    return self.has_class(a, c, same);
                }
                let (a, b) = (self.class_text(a), self.class_text(b));
                infix(if same { "==" } else { "!=" }, CMP, &a, &b)
            }
        }
    }

    /// The class of `object` where its type says it and reading it has no effects and cannot
    /// fail: the object is a struct, not an enum of classes.
    fn known_class(&self, object: &Expr) -> Option<usize> {
        if has_effects(object) || self.object_may_fail(object) || self.enum_of(object).is_some() {
            return None;
        }
        // An enum's constant with a body is of the body's class.
        self.struct_class(object).filter(|&c| !self.has_bodies(c))
    }

    /// Whether the class of `object` is class `c` (`!same`: is not).
    fn has_class(&mut self, object: &'a Expr, c: usize, same: bool) -> Code {
        // Of an enum whose constants with a body are of the body's class, the names are compared.
        if self.has_bodies(c) {
            let name = Code::atom(rust_string(&self.java_class_name(c, false)));
            let text = self.class_text(object);
            return infix(if same { "==" } else { "!=" }, CMP, &text, &name);
        }
        let Some(t) = self.enum_of(object) else {
            let is = self.struct_class(object) == Some(c);
            return self.after_object(object, if is == same { "true" } else { "false" });
        };
        if !self.is_enum_variant(t, c) {
            return self.after_object(object, if same { "false" } else { "true" });
        }
        let (obj, tag) = (self.object(object), self.class_tag(c));
        let not = if same { "" } else { "!" };
        Code::new(
            format!("{not}matches!({}.0.tag(), {tag})", super::dispatch::operand(&obj)),
            if same { ATOM } else { UNARY },
        )
    }

    /// The name of the class of `object`, a `&str`: `"Main$Dog"`, or a `match` of its enum.
    pub(super) fn class_text(&mut self, object: &'a Expr) -> Code {
        let Some(t) = self.enum_of(object) else {
            let c = self.struct_class(object).expect("an object has a class");
            if self.has_bodies(c) {
                let value = self.object(object);
                return Code::atom(self.constant_class_name(c, &value, false));
            }
            let name = rust_string(&self.java_class_name(c, false));
            return self.after_object(object, &name);
        };
        let obj = self.object(object);
        self.need(Need::RuntimeClassName(t));
        Code::atom(super::class_values::method_on(&obj, "runtime_class_name"))
    }

    /// `value`, once `object` has run where it has effects or may fail: a null throws
    /// `NullPointerException`, as do the objects it reads, in the checked form where a `catch`
    /// sees it.
    pub(super) fn after_object(&mut self, object: &'a Expr, value: &str) -> Code {
        if self.nullable(object) {
            let check = self.null_check(object).text;
            return Code::new(format!("{{ {check}; {value} }}"), LOWEST);
        }
        if has_effects(object) {
            let text = self.expr(object).text;
            return Code::new(format!("{{ {text}; {value} }}"), LOWEST);
        }
        if self.object_may_fail(object) {
            let text = self.expr(object).text;
            return Code::new(format!("{{ let _ = {text}; {value} }}"), LOWEST);
        }
        Code::borrow_or_atom(value)
    }

    /// Whether `object` is null or holds an operation whose failure a `catch` or a `finally`
    /// sees.
    fn object_may_fail(&self, object: &Expr) -> bool {
        let mut fails = self.nullable(object);
        visit_expr(object, &mut |x| {
            fails |= self.facts.null_checked.contains_key(&x.id)
                || self.facts.catching.contains_key(&x.id)
                || self.facts.raising.contains(&x.id)
        });
        fails
    }
}

/// The class that a `ClassLiteralName` names.
fn literal_class(e: &Expr) -> Option<usize> {
    match e.kind {
        ExprKind::Builtin(Builtin::ClassLiteralName { class, .. }, _) => Some(class),
        _ => None,
    }
}

/// The object of `ClassName(o)`.
fn object_of(e: &Expr) -> &Expr {
    match &e.kind {
        ExprKind::Builtin(Builtin::ClassName { .. }, args) => &args[0],
        _ => unreachable!("a class is a ClassName or a ClassLiteralName"),
    }
}
