//! A regular expression call of `String` whose pattern is not a constant, in a program translated
//! with the JDK's sources: the JDK's own `Pattern` runs it, as the JDK's `String` calls it
//! (`program::REGEX_CALLS`), so that what it throws for a malformed pattern or replacement, and
//! the message, are Java's. `s.replaceAll(regex, replacement)` becomes
//!
//! ```text
//! { String receiver$ = s; String argument$ = regex; String argument$ = replacement;
//!   <s is checked for null, as `String.replaceAll(String, String)`>;
//!   yield Pattern.compile(argument$).matcher(receiver$).replaceAll(argument$); }
//! ```
//!
//! which computes the receiver and the arguments, then throws for a null receiver, as Java's call
//! does, before the pattern compiles. `s.matches(regex)` is `Pattern.matches(regex, s)` and
//! `s.split(regex, limit)` is `Pattern.compile(regex).split(s, limit)` (with a `limit` of 0 for
//! `s.split(regex)`; the JDK's shortcut for a pattern of one plain character splits as the
//! pattern does). A constant pattern is the runtime's (`lower::strings`), which is checked when
//! the program is translated, and so is every pattern where the program has no `Pattern`.

use super::*;

impl<'t> Lowering<'t> {
    /// `receiver.name(args)`, a call of `String`'s regular expression method `op`, as the JDK's
    /// `Pattern` runs it (see the module's comment); `None` where the program has no `Pattern`.
    pub(super) fn jdk_regex(
        &self,
        node: Node<'t>,
        op: StringOp,
        receiver: Node<'t>,
        args: &mut Vec<Expr>,
    ) -> Result<Option<ExprKind>, Error> {
        let name = match op {
            StringOp::Matches => "matches",
            StringOp::ReplaceAll => "replaceAll",
            StringOp::ReplaceFirst => "replaceFirst",
            StringOp::Split => "split",
            _ => return Ok(None),
        };
        let (_, calls) =
            crate::program::REGEX_CALLS.iter().find(|(m, _)| *m == name).expect("a regular expression call");
        let mut resolved = Vec::new();
        for &(owner, method, sig) in calls.iter() {
            let found = self
                .methods
                .iter()
                .position(|m| self.types[m.owner].javac == owner && m.name == method && m.sig == sig);
            let Some(m) = found else { return Ok(None) };
            resolved.push(m);
        }
        let pos = node.pos();
        let params = match (op, args.len()) {
            (StringOp::Matches, _) => "String",
            (StringOp::Split, 1) => "String",
            (StringOp::Split, _) => "String, int",
            _ => "String, String",
        };
        // The receiver, then the arguments, each held in a local.
        let receiver = self.expr(receiver)?;
        let mut body = Vec::new();
        let mut hold = |value: Expr, prefix: &str| {
            let (ty, name) = (value.ty, format!("{prefix}{}", value.id));
            body.push(Stmt::Local { ty, name: name.clone(), init: Some(value), pos });
            move |l: &Self| l.synth(ExprKind::Var(name.clone()), pos, ty)
        };
        let receiver = hold(receiver, super::RECEIVER);
        let held: Vec<_> = args.drain(..).map(|a| hold(a, super::argument_order::ARGUMENT)).collect();
        let check =
            ExprKind::Builtin(Builtin::String(StringOp::Receiver(name, params)), vec![receiver(self)]);
        body.push(Stmt::Expr(self.synth(check, pos, Type::Void)));
        let arg = |i: usize| held.get(i).map(|h| h(self));
        let call = |m: usize, args: Vec<Expr>| -> Result<(ExprKind, Type), Error> {
            let info = &self.methods[m];
            let args = self.convert_args(Some(m), args)?;
            let ret = info.node.child("returnType")?;
            // `split`'s `String[]` is an `Object[]` where it is covariant (`covariant`).
            let ty = self.covariant_array(self.declared_type(ret, ret.java_type(), info.owner, Some(m))?);
            Ok((ExprKind::Call { name: info.name.to_string(), args, resolved: Some(m) }, ty))
        };
        let on = |object: (ExprKind, Type), m: usize, args: Vec<Expr>| -> Result<(ExprKind, Type), Error> {
            let (ExprKind::Call { name, args, resolved }, ty) = call(m, args)? else { unreachable!() };
            let object = self.synth(object.0, pos, object.1);
            Ok((
                ExprKind::MethodCall { receiver: Box::new(object), name, args, resolved, is_super: false },
                ty,
            ))
        };
        let regex = arg(0).expect("a pattern");
        let (kind, ty) = match op {
            StringOp::Matches => call(resolved[0], vec![regex, receiver(self)])?,
            StringOp::Split => {
                let limit = arg(1).unwrap_or_else(|| self.synth(ExprKind::Int(0), pos, Type::Int));
                let pattern = call(resolved[0], vec![regex])?;
                on(pattern, resolved[1], vec![receiver(self), limit])?
            }
            _ => {
                let replacement = arg(1).expect("a replacement");
                let pattern = call(resolved[0], vec![regex])?;
                let matcher = on(pattern, resolved[1], vec![receiver(self)])?;
                on(matcher, resolved[2], vec![replacement])?
            }
        };
        body.push(Stmt::Yield(self.synth(kind, pos, ty)));
        Ok(Some(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        }))
    }
}
