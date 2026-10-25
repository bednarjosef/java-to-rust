//! The natives of the translator's `java.lang.reflect.TranslatedProxies`
//! (`java/overlay/java/lang/reflect/TranslatedProxies.java`) over the proxy classes that
//! reachability found (`Reach::proxies`, `program/reach/proxies.rs`): `interfaces0()`, the lists
//! of interfaces as arrays of class literals, `classes0()`, the class literals of their proxy
//! classes in the same order, `packages0()`, the package of the interfaces of each that are not
//! public (`null` when all are), and `newInstance0(index, h)`, a `switch` over the index that makes
//! an object of the class at that index with its constructor.

use super::*;

const PROXIES: &str = "java.lang.reflect.TranslatedProxies";

impl<'t> Lowering<'t> {
    /// Whether type `t` is one of the proxy classes that reachability wrote, whose code stands for
    /// what the JDK's `ProxyGenerator` writes (`Method::jdk_code`).
    pub(super) fn is_proxy_class(&self, t: usize) -> bool {
        let name = &self.types[t].name;
        self.reach.is_some_and(|r| r.proxies.values().any(|made| &made.class == name))
    }

    /// A call of one of these natives, with its arguments; `None` for any other call.
    pub(super) fn proxy_native(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        args: &mut Vec<Expr>,
    ) -> Result<Option<ExprKind>, Error> {
        if symbol.owner != PROXIES
            || !matches!(symbol.name, "interfaces0" | "classes0" | "packages0" | "newInstance0")
        {
            return Ok(None);
        }
        let pos = node.pos();
        // The proxy classes, each with its list of interfaces, as indexes into `types`.
        let mut proxies: Vec<(usize, Vec<usize>, Option<&str>)> = Vec::new();
        for (list, made) in self.reach.map(|r| &r.proxies).into_iter().flatten() {
            let interfaces: Option<Vec<usize>> = list.iter().map(|i| self.type_index(i)).collect();
            if let (Some(class), Some(interfaces)) = (self.type_index(&made.class), interfaces) {
                proxies.push((class, interfaces, made.hidden.as_deref()));
            }
        }
        let literal = |t: usize| {
            self.synth(
                ExprKind::Builtin(Builtin::ClassLiteral(self.self_type(t)), Vec::new()),
                pos,
                Type::JavaClass,
            )
        };
        let array = |classes: Vec<Expr>, ty: Type| {
            let kind = if classes.is_empty() {
                ExprKind::NewArray(vec![self.synth(ExprKind::Int(0), pos, Type::Int)])
            } else {
                ExprKind::ArrayLit(classes.into_iter().map(|c| self.convert(c, ty.element())).collect())
            };
            self.synth(kind, pos, ty)
        };
        let ret = self.ty(node, symbol.ty.rsplit(')').next().unwrap_or_default())?;
        Ok(Some(match symbol.name {
            "interfaces0" => {
                let lists = proxies
                    .iter()
                    .map(|(_, interfaces, _)| {
                        array(interfaces.iter().map(|&i| literal(i)).collect(), ret.element())
                    })
                    .collect();
                array(lists, ret).kind
            }
            "classes0" => array(proxies.iter().map(|&(class, _, _)| literal(class)).collect(), ret).kind,
            "packages0" => {
                let packages = proxies
                    .iter()
                    .map(|&(_, _, hidden)| match hidden {
                        Some(package) => self.synth(ExprKind::Str(package.to_string()), pos, Type::String),
                        None => self.synth(ExprKind::Null, pos, Type::Null),
                    })
                    .collect();
                array(packages, ret).kind
            }
            _ => {
                let [index, handler] = <[Expr; 2]>::try_from(std::mem::take(args)).map_err(|_| Error {
                    pos,
                    message: "`TranslatedProxies.newInstance0` with other arguments".to_string(),
                })?;
                let object = Type::class(OBJECT);
                // The handler in a local, which the case of each class reads.
                let local = self.fresh_name("handler");
                let handler_ty = handler.ty;
                let var = || self.synth(ExprKind::Var(local.clone()), pos, handler_ty);
                let mut cases = Vec::new();
                for (slot, &(class, _, _)) in proxies.iter().enumerate() {
                    let Some(ctor) = (0..self.methods.len())
                        .find(|&m| self.methods[m].owner == class && self.methods[m].name == "<init>")
                    else {
                        continue;
                    };
                    let param = self.param_types(ctor)?[0];
                    let made =
                        ExprKind::New { class, args: vec![self.convert(var(), param)], ctor: Some(ctor) };
                    let made = self.convert(self.synth(made, pos, self.self_type(class)), object);
                    let label = self.synth(ExprKind::Int(slot as i32), pos, Type::Int);
                    cases.push(Case { labels: vec![CaseLabel::Value(label)], body: vec![Stmt::Yield(made)] });
                }
                let null = self.synth(ExprKind::Null, pos, Type::Null);
                cases.push(Case { labels: vec![CaseLabel::Default], body: vec![Stmt::Yield(null)] });
                let switch = ExprKind::Switch { selector: Box::new(index), cases };
                let body = vec![
                    Stmt::Local { ty: handler_ty, name: local.clone(), init: Some(handler), pos },
                    Stmt::Yield(self.synth(switch, pos, object)),
                ];
                let zero = self.synth(ExprKind::Int(0), pos, Type::Int);
                ExprKind::Switch {
                    selector: Box::new(zero),
                    cases: vec![Case { labels: vec![CaseLabel::Default], body }],
                }
            }
        }))
    }
}
