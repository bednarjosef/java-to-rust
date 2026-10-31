//! `StackWalker.walk(function)`: the function gets a stream of the frames on the call stack. A
//! translation has no call stack (frames stay out of the translation, as they do of stack traces),
//! but it knows the top frame, the frame of the method that calls `walk`: the class whose code
//! that method is, the method's name as the class file has it (`<init>` for a constructor), the
//! source file of the class and the line where the call starts. So a call of `walk` in a method's
//! own body is a call of the translator's `walkFrom(function, frame)` (`java/Standard.java`), whose
//! stream holds that frame alone; elsewhere (a lambda's body, a field's initializer, code of a
//! class without a source file) the stream is empty. A walk that looks for the first frame, or
//! for the first one of the walking class, so sees what Java sees; one that looks past the walking
//! method finds nothing, the closest answer the translation has.

use super::*;

/// The translator's frame class, which lowering makes of what it knows.
pub(super) const KNOWN_FRAME: &str = "java.lang.StackWalker.KnownFrame";
/// The signature of `KnownFrame`'s constructor: the declaring class, the method's name, the file's
/// name and the line.
pub(crate) const KNOWN_FRAME_CONSTRUCTOR: &str =
    "(java.lang.Class<?>,java.lang.String,java.lang.String,int)void";

impl<'t> Lowering<'t> {
    /// `walker.walk(function)`, as `walker.walkFrom(function, frame)` with the frame of the method
    /// that walks, or null where lowering does not know it.
    pub(super) fn stack_walk(
        &self,
        node: Node<'t>,
        receiver: Option<Node<'t>>,
        mut args: Vec<Expr>,
    ) -> Result<ExprKind, Error> {
        let walker = self.type_index("java.lang.StackWalker");
        let walk_from = walker.and_then(|w| {
            let methods = self.methods.iter().enumerate();
            methods.filter(|(_, m)| m.owner == w && m.name == "walkFrom").map(|(i, _)| i).next()
        });
        let Some(walk_from) = walk_from else {
            return unsupported(node, "`StackWalker.walk` outside a whole program");
        };
        let frame_type = Type::class("java.lang.StackWalker.StackFrame");
        let pos = node.pos();
        let top = match self.walking_frame(node)? {
            Some(frame) => self.convert(frame, frame_type),
            None => self.convert(self.synth(ExprKind::Null, pos, Type::Null), frame_type),
        };
        args.push(top);
        let args = self.convert_args(Some(walk_from), args)?;
        let receiver = match receiver {
            Some(r) => self.receiver(r, "java.lang.StackWalker")?,
            None => self.this(node),
        };
        Ok(ExprKind::MethodCall {
            receiver: Box::new(receiver),
            name: "walkFrom".to_string(),
            args,
            resolved: Some(walk_from),
            is_super: false,
        })
    }

    /// `new KnownFrame(C.class, "method", "File.java", line)` for the method whose own body has
    /// the call `node`, `None` where lowering does not know the frame.
    fn walking_frame(&self, node: Node<'t>) -> Result<Option<Expr>, Error> {
        let current = self.current.get();
        let Some(m) = self.method_ctx.get().filter(|&m| self.methods[m].owner == current) else {
            return Ok(None);
        };
        if self.lambda_of_method(m).is_some() || self.lambda_of_type(current).is_some() {
            return Ok(None);
        }
        let Some(file) = self.source_file(current) else { return Ok(None) };
        let Some(frame) = self.type_index(KNOWN_FRAME) else { return Ok(None) };
        let ctor = self.methods.iter().position(|c| c.owner == frame && c.name == "<init>");
        let Some(ctor) = ctor else { return Ok(None) };
        let pos = node.pos();
        let class = self.synth(
            ExprKind::Builtin(Builtin::ClassLiteral(self.self_type(current)), Vec::new()),
            pos,
            Type::JavaClass,
        );
        let text = |s: &str| self.synth(ExprKind::Str(s.to_string()), pos, Type::String);
        let line = i32::try_from(pos.line).unwrap_or(-1);
        let args = vec![
            class,
            text(self.methods[m].name),
            text(file),
            self.synth(ExprKind::Int(line), pos, Type::Int),
        ];
        let new = ExprKind::New { class: frame, args, ctor: Some(ctor) };
        Ok(Some(self.synth(new, pos, Type::class(KNOWN_FRAME))))
    }

    /// The name of the source file of class `t`, as its class file has it: that of the unit that
    /// declares it, or of the class whose code declares it.
    fn source_file(&self, t: usize) -> Option<&'t str> {
        if let Some(&file) = self.source_files.borrow().get(&t) {
            return Some(file);
        }
        self.nesting(t).and_then(|n| self.source_file(n.enclosing))
    }
}
