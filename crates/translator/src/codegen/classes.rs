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

    /// The class of `object` where its type says it and reading it has no effects: the object
    /// is a struct, not an enum of classes.
    fn known_class(&self, object: &Expr) -> Option<usize> {
        if has_effects(object) || self.enum_of(object).is_some() {
            return None;
        }
        self.struct_class(object)
    }

    /// Whether the class of `object` is class `c` (`!same`: is not).
    fn has_class(&mut self, object: &'a Expr, c: usize, same: bool) -> Code {
        let Some(t) = self.enum_of(object) else {
            let is = self.struct_class(object) == Some(c);
            return self.after_effects(object, if is == same { "true" } else { "false" });
        };
        if !self.enum_variants(t).contains(&c) {
            return self.after_effects(object, if same { "false" } else { "true" });
        }
        let (obj, variant) = (self.object(object), self.variant(t, c));
        let not = if same { "" } else { "!" };
        Code::new(format!("{not}matches!({obj}, {variant}(_))"), if same { ATOM } else { UNARY })
    }

    /// The name of the class of `object`, a `&str`: `"Main$Dog"`, or a `match` of its enum.
    pub(super) fn class_text(&mut self, object: &'a Expr) -> Code {
        let Some(t) = self.enum_of(object) else {
            let c = self.struct_class(object).expect("an object has a class");
            let name = rust_string(&self.java_class_name(c, false));
            return self.after_effects(object, &name);
        };
        let obj = self.object(object);
        let arms: Vec<String> = self
            .enum_variants(t)
            .into_iter()
            .map(|c| format!("{}(_) => {}", self.variant(t, c), rust_string(&self.java_class_name(c, false))))
            .collect();
        // A `match` (or a block) that starts a statement would be one, so it goes in parentheses.
        Code::new(super::class_values::variant_match(&obj, &arms), LOWEST)
    }

    /// `value`, once `object` has run when it has effects.
    fn after_effects(&mut self, object: &'a Expr, value: &str) -> Code {
        if has_effects(object) {
            let text = self.expr(object).text;
            return Code::new(format!("{{ {text}; {value} }}"), LOWEST);
        }
        Code::atom(value)
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
