// Repairs decompiled Java that javac rejects, with edits that keep the bytecode's behaviour (see
// docs/minecraft-server.md, "Repairing what the decompiler left out").
//
//   java --add-exports ... scripts/RepairDecompile.java ROOT CLASSPATH ROUNDS LOG [CLASSES]
//
// CLASSES is the jar that the tree was decompiled from; without it, no cast down is made.
//
// Compiles every .java file under ROOT with javac's tree API, reads each error's kind, position
// and types, edits the files in place, and compiles again, up to ROUNDS times. Only these edits
// are made, all of which the class files' bytecode already behaves as:
//
// - a cast to the type javac expects, where the found type is not convertible but its erasure is
//   a subtype of the expected type's erasure: the cast is unchecked and javac emits no checkcast
//   for it, which is what the original source compiled to (`(List<Component>) (List<?>) list`),
//   or a cast to the raw type where the expected type holds a captured wildcard, which source
//   cannot write (`(EntityType) type`);
// - a cast down to the type javac expects, where its erasure is a subtype of the found type's and
//   the class file of the class the expression is in has a checkcast to it: the one that javac
//   emitted for the original's generic call or field, whose type the decompiler wrote as the
//   erased one (`(EntityHitResult) e`);
// - type arguments for a diamond, copied from the declared type of the variable or method result
//   it initializes or returns when that is the same class (inference only, no bytecode);
// - a diamond for an argument `new C(..)` that makes a generic class raw, of a call whose type
//   variables javac cannot infer (`new Info()` for the original's `new Info<>()`), tried before a
//   cast at the same error;
// - in a for-each loop over a raw collection whose elements the class file casts to the loop
//   variable's class, the collection cast to `Iterable` of that class;
// - the type argument of a generic call whose result's type argument a call on it returns
//   (`.<InteractionResult>map(..).orElse(PASS)`), written as the declared type that the whole
//   is returned as or initializes;
// - a lambda or method reference for an interface that is not functional, cast to the one
//   interface extending it that the class file makes lambdas for (`(ForValueType<T>) () -> codec`);
// - a cast or a call's type arguments that name a type variable of the class in a static method
//   (`Lens.<S, T, A, B>unbox(l)`, the decompiler's names for the method's own variables) left out;
// - a type variable's bound that names a member class of the variable's own name
//   (`Mu extends Mu`) written as the class's member (`Mu extends CartesianLike.Mu`);
// - a cast whose last type argument the decompiler left empty (`(Function<Object, >) a -> x`)
//   made a cast to the raw type;
// - an unresolved variable in a catch block renamed to the catch parameter that encloses every
//   use of the name and is not used itself (the decompiler declared `e` and used `ex`);
// - a parameter whose type the decompiler erased, in a method that then overrides nothing and
//   leaves its class abstract, written with the type of the method of a supertype that has the
//   same erasure (`encode(R input, ..)` for `encode(Object input, ..)`);
// - in an argument, or in an expression whose type variables javac cannot infer, a call's type
//   arguments that name `Object` (`Either.<Object, R>unbox(e)`, types the decompiler could not
//   name) left out, and a cast to a type variable erased to `Object` that is a lambda's whole
//   body (`a -> (T) f.apply(a)`) left out (no checkcast either way);
// - `new Outer.Inner<>(..)` of an inner class of a generic class that the enclosing class is or
//   extends, which javac rejects with the raw qualifier, written `new Inner<>(..)`.
//
// A file whose errors outside imports and annotations do not go down after a round gets its
// previous text back. Each edit is written to LOG.
import com.sun.source.tree.*;
import com.sun.source.util.*;
import com.sun.tools.javac.api.ClientCodeWrapper;
import com.sun.tools.javac.api.JavacTaskImpl;
import com.sun.tools.javac.code.Kinds;
import com.sun.tools.javac.code.Symbol;
import com.sun.tools.javac.code.Type;
import com.sun.tools.javac.code.TypeTag;
import com.sun.tools.javac.code.Types;
import com.sun.tools.javac.tree.JCTree;
import com.sun.tools.javac.tree.TreeInfo;
import com.sun.tools.javac.util.Context;
import com.sun.tools.javac.util.JCDiagnostic;
import java.io.*;
import java.lang.classfile.ClassFile;
import java.lang.classfile.CodeModel;
import java.lang.classfile.Opcode;
import java.lang.classfile.instruction.InvokeDynamicInstruction;
import java.lang.classfile.instruction.TypeCheckInstruction;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.stream.*;
import java.util.zip.ZipFile;
import javax.tools.*;

public class RepairDecompile {
    record Edit(int start, int end, String text, String what) {}

    // An error, with whether it is in an import or an annotation (which the translator ignores).
    record Error(JCDiagnostic diag, boolean ignored) {}

    @ClientCodeWrapper.Trusted
    static class Collector implements DiagnosticListener<JavaFileObject> {
        final List<JCDiagnostic> errors = new ArrayList<>();

        public void report(Diagnostic<? extends JavaFileObject> d) {
            if (d.getKind() == Diagnostic.Kind.ERROR && d instanceof JCDiagnostic jc) errors.add(jc);
        }
    }

    static String classpath;
    static Path root;
    static PrintWriter log;
    // The jar of the tree's classes, and the types each class file checks casts to, by name.
    static ZipFile classes;
    static Map<String, ClassCode> classCodes = new HashMap<>();

    public static void main(String[] args) throws Exception {
        root = Path.of(args[0]).toAbsolutePath();
        classpath = args[1];
        int rounds = Integer.parseInt(args[2]);
        log = new PrintWriter(Files.newBufferedWriter(Path.of(args[3]), StandardCharsets.UTF_8), true);
        if (args.length > 4) classes = new ZipFile(args[4]);
        List<Path> all;
        try (Stream<Path> files = Files.walk(root)) {
            all = files.filter(p -> p.toString().endsWith(".java")).sorted().toList();
        }
        Map<Path, List<Error>> errors = errorsOf(all);
        report("before", errors);
        Set<Path> pending = new TreeSet<>(errors.keySet());
        // Edits that did not lower their file's errors, which are not tried again.
        Map<Path, Set<Edit>> failed = new HashMap<>();
        for (int round = 1; round <= rounds && !pending.isEmpty(); round++) {
            Map<Path, String> before = new HashMap<>();
            Map<Path, Long> counted = new HashMap<>();
            Map<Path, List<Edit>> edits = new TreeMap<>();
            compile(List.copyOf(pending), (task, units) -> {
                for (CompilationUnitTree unit : units) {
                    Path file = Path.of(unit.getSourceFile().toUri());
                    List<Error> found = errors.getOrDefault(file, List.of());
                    List<Edit> fileEdits = new ArrayList<>(repairs(task, unit, found));
                    fileEdits.removeAll(failed.getOrDefault(file, Set.of()));
                    if (!fileEdits.isEmpty()) edits.put(file, fileEdits);
                }
            }, errors);
            for (var entry : edits.entrySet()) {
                Path file = entry.getKey();
                String text = Files.readString(file);
                before.put(file, text);
                counted.put(file, errors.get(file).stream().filter(e -> !e.ignored()).count());
                Files.writeString(file, apply(text, entry.getValue()));
                for (Edit edit : entry.getValue())
                    log.printf("round %d %s:%d %s%n", round, root.relativize(file), line(text, edit.start()), edit.what());
            }
            if (edits.isEmpty()) break;
            Map<Path, List<Error>> after = errorsOf(List.copyOf(edits.keySet()));
            for (Path file : edits.keySet()) {
                long now = after.getOrDefault(file, List.of()).stream().filter(e -> !e.ignored()).count();
                if (now >= counted.get(file)) {
                    Files.writeString(file, before.get(file));
                    failed.computeIfAbsent(file, f -> new HashSet<>()).addAll(edits.get(file));
                    log.printf("round %d %s: reverted (%d errors before, %d after)%n", round, root.relativize(file),
                        counted.get(file), now);
                } else {
                    errors.remove(file);
                    if (after.containsKey(file)) errors.put(file, after.get(file));
                }
            }
            pending = new TreeSet<>(edits.keySet());
            pending.retainAll(errors.keySet());
        }
        report("after", errorsOf(all));
    }

    static void report(String when, Map<Path, List<Error>> errors) {
        long total = errors.values().stream().mapToLong(List::size).sum();
        long counted = errors.values().stream().flatMap(List::stream).filter(e -> !e.ignored()).count();
        String line = String.format("%s: %d errors in %d files, %d outside imports and annotations", when, total,
            errors.size(), counted);
        System.out.println(line);
        log.println(line);
    }

    interface Analyzed {
        void accept(JavacTask task, List<CompilationUnitTree> units) throws IOException;
    }

    // Compiles the files (attributes them, without writing classes) against the classpath, the rest
    // of the tree being the source path. Returns their errors by file; with `then`, also runs it on
    // the analyzed trees, with the errors of `into` replaced by the new ones first.
    static Map<Path, List<Error>> compile(List<Path> files, Analyzed then, Map<Path, List<Error>> into)
            throws IOException {
        JavaCompiler javac = ToolProvider.getSystemJavaCompiler();
        Collector collector = new Collector();
        try (StandardJavaFileManager manager = javac.getStandardFileManager(collector, null, StandardCharsets.UTF_8)) {
            List<String> options = List.of("-proc:none", "-implicit:none", "-nowarn", "-Xmaxerrs", "1000000",
                "-encoding", "UTF-8", "-cp", classpath, "-sourcepath", root.toString());
            JavacTask task = (JavacTask) javac.getTask(null, manager, collector, options, null,
                manager.getJavaFileObjectsFromPaths(files));
            List<CompilationUnitTree> units = new ArrayList<>();
            task.parse().forEach(units::add);
            task.analyze();
            Map<CompilationUnitTree, Set<Tree>> ignoredTrees = new HashMap<>();
            Map<Path, List<Error>> result = new TreeMap<>();
            Set<Path> roots = new HashSet<>(files);
            Map<Path, CompilationUnitTree> unitOf = new HashMap<>();
            for (CompilationUnitTree unit : units) unitOf.put(Path.of(unit.getSourceFile().toUri()), unit);
            for (JCDiagnostic diag : collector.errors) {
                if (diag.getSource() == null) continue;
                Path file = Path.of(diag.getSource().toUri());
                if (!roots.contains(file)) continue;
                result.computeIfAbsent(file, f -> new ArrayList<>())
                    .add(new Error(diag, inImportOrAnnotation(unitOf.get(file), diag.getStartPosition())));
            }
            if (then != null) {
                for (Path file : roots) {
                    into.remove(file);
                    if (result.containsKey(file)) into.put(file, result.get(file));
                }
                then.accept(task, units);
            }
            return result;
        }
    }

    static Map<Path, List<Error>> errorsOf(List<Path> files) throws IOException {
        return compile(files, null, null);
    }

    static boolean inImportOrAnnotation(CompilationUnitTree unit, long pos) {
        if (unit == null) return false;
        for (ImportTree imp : unit.getImports()) {
            JCTree tree = (JCTree) imp;
            if (tree.getStartPosition() <= pos && pos <= tree.getEndPosition(((JCTree.JCCompilationUnit) unit).endPositions))
                return true;
        }
        boolean[] found = {false};
        new TreeScanner<Void, Void>() {
            public Void visitAnnotation(AnnotationTree node, Void v) {
                JCTree tree = (JCTree) node;
                if (tree.getStartPosition() <= pos
                        && pos <= tree.getEndPosition(((JCTree.JCCompilationUnit) unit).endPositions))
                    found[0] = true;
                return null;
            }
        }.scan(unit, null);
        return found[0];
    }

    // The edits for one file's errors.
    static List<Edit> repairs(JavacTask task, CompilationUnitTree unit, List<Error> errors) {
        Context context = ((JavacTaskImpl) task).getContext();
        Types types = Types.instance(context);
        var elements = com.sun.tools.javac.model.JavacElements.instance(context);
        java.util.function.Function<String, Symbol.ClassSymbol> named =
            binary -> (Symbol.ClassSymbol) elements.getTypeElement(binary.replace('$', '.'));
        JCTree.JCCompilationUnit cu = (JCTree.JCCompilationUnit) unit;
        List<Edit> edits = new ArrayList<>();
        Set<Integer> renamed = new HashSet<>();
        for (Error error : errors) {
            if (error.ignored()) continue;
            JCDiagnostic diag = error.diag();
            if (diag.getCode().equals("compiler.err.illegal.start.of.type")) {
                Edit edit = rawCastOfMissingArgument(cu, (int) diag.getPosition());
                if (edit != null) edits.add(edit);
                continue;
            }
            if (diag.getCode().equals("compiler.err.cyclic.inheritance")) {
                edits.addAll(qualifiedBounds(cu));
                continue;
            }
            JCTree tree = diag.getDiagnosticPosition() == null ? null : diag.getDiagnosticPosition().getTree();
            if (tree == null) continue;
            TreePath path = TreePath.getPath(unit, tree);
            if (path == null) continue;
            try {
                switch (diag.getCode()) {
                    case "compiler.err.prob.found.req" -> {
                        JCDiagnostic detail = inconvertible(diag);
                        List<Edit> diamonds = rawArguments(cu, path);
                        JCDiagnostic notFunctional = detail("compiler.misc.not.a.functional.intf", diag);
                        if (notFunctional != null && notFunctional.getArgs()[0] instanceof Symbol.ClassSymbol written) {
                            Edit edit = functionalCast(types, cu, tree, targetOf(path, written, types), named);
                            if (edit != null) edits.add(edit);
                        } else if (!diamonds.isEmpty()) {
                            edits.addAll(diamonds);
                        } else if (detail != null) {
                            Type found = (Type) detail.getArgs()[0], expected = (Type) detail.getArgs()[1];
                            // An argument of a call whose type arguments the decompiler gave as
                            // `Object`: those go first, and inference may then take the argument.
                            Edit edit = path.getParentPath().getLeaf() instanceof JCTree.JCMethodInvocation call
                                && call.args.contains(tree) ? objectTypeArguments(cu, call) : null;
                            if (edit == null) edit = witness(cu, path);
                            if (edit == null) edit = cast(types, cu, tree, path.getParentPath().getLeaf(), found, expected);
                            if (edit != null) edits.add(edit);
                        } else if (inference(diag)) {
                            edits.addAll(inferenceHints(types, cu, tree));
                        }
                    }
                    case "compiler.err.does.not.override.abstract" -> {
                        // The class's methods of the abstract method's name, one of which the
                        // decompiler wrote with an erased parameter type.
                        JCTree.JCClassDecl decl = tree instanceof JCTree.JCClassDecl c ? c
                            : tree instanceof JCTree.JCNewClass created ? created.def : null;
                        if (decl != null && diag.getArgs()[1] instanceof Symbol.MethodSymbol missing)
                            for (JCTree def : decl.defs)
                                if (def instanceof JCTree.JCMethodDecl method && method.name == missing.name)
                                    edits.addAll(overriddenParameters(types, cu, TreePath.getPath(unit, method)));
                    }
                    case "compiler.err.improperly.formed.type.inner.raw.param" -> {
                        Edit edit = innerOfRawQualifier(types, cu, path);
                        if (edit != null) edits.add(edit);
                    }
                    case "compiler.err.cant.apply.symbol" -> edits.addAll(rawArguments(cu, path));
                    case "compiler.err.non-static.cant.be.ref" -> {
                        Edit edit = diag.getArgs()[0] == Kinds.KindName.TYPEVAR ? withoutOuterVariable(cu, path) : null;
                        if (edit != null) edits.add(edit);
                    }
                    case "compiler.err.cant.apply.diamond.1" -> {
                        Edit edit = diamond(cu, path);
                        if (edit != null) edits.add(edit);
                    }
                    case "compiler.err.cant.resolve.location" -> {
                        if (diag.getArgs()[0] == Kinds.KindName.VAR && renamed.add(tree.getStartPosition()))
                            edits.addAll(renameCatchVariable(cu, path, diag.getArgs()[1].toString(), renamed));
                    }
                    default -> {}
                }
            } catch (Unprintable e) {
                // a type that source cannot name (a captured or intersection type, a local class)
            }
        }
        return edits.stream().distinct().toList();
    }

    static class Unprintable extends Exception {}

    // Whether an error is one of inference: a call's type variables that javac cannot infer, or
    // whose bounds disagree.
    static boolean inference(JCDiagnostic diag) {
        for (Object arg : diag.getArgs()) {
            if (!(arg instanceof JCDiagnostic detail)) continue;
            String code = detail.getCode();
            if (code.startsWith("compiler.misc.infer.") || code.startsWith("compiler.misc.incompatible.")
                    && code.endsWith("bounds")) return true;
            if (inference(detail)) return true;
        }
        return false;
    }

    // Inside the expression of an inference error (`expr`), what the decompiler wrote that
    // inference then has to meet, left out: a cast to a type variable whose erasure is `Object`
    // that is a lambda's whole body (`e -> (T) f.apply(e)`, a variable of the enclosing method's
    // that the decompiler chose for the erased result), and the type arguments of a call that
    // give `Object` (`Either.<Object, R>unbox(a)`, a type it could not name). Such a cast makes no
    // checkcast, and type arguments are inference only.
    static List<Edit> inferenceHints(Types types, JCTree.JCCompilationUnit cu, JCTree expr) {
        List<Edit> edits = new ArrayList<>();
        new TreeScanner<Void, Void>() {
            public Void visitLambdaExpression(LambdaExpressionTree node, Void v) {
                if (((JCTree.JCLambda) node).body instanceof JCTree.JCTypeCast cast && cast.clazz.type != null
                        && cast.clazz.type.hasTag(TypeTag.TYPEVAR) && !(cast.clazz.type instanceof Type.CapturedType)
                        && types.erasure(cast.clazz.type).tsym.getQualifiedName().contentEquals("java.lang.Object")) {
                    int start = cast.getStartPosition(), body = cast.expr.getStartPosition();
                    edits.add(new Edit(start, body, "", "no cast " + source(cu, start, body).strip() + " of a lambda's result"));
                }
                return super.visitLambdaExpression(node, v);
            }

            public Void visitMethodInvocation(MethodInvocationTree node, Void v) {
                Edit edit = objectTypeArguments(cu, (JCTree.JCMethodInvocation) node);
                if (edit != null) edits.add(edit);
                return super.visitMethodInvocation(node, v);
            }
        }.scan(expr, null);
        return edits;
    }

    // The type arguments of `call` left out, where one of them is `Object`.
    static Edit objectTypeArguments(JCTree.JCCompilationUnit cu, JCTree.JCMethodInvocation call) {
        if (!(call.meth instanceof JCTree.JCFieldAccess select) || call.typeargs.stream().noneMatch(
                t -> t.type != null && t.type.hasTag(TypeTag.CLASS)
                    && t.type.tsym.getQualifiedName().contentEquals("java.lang.Object"))) return null;
        int after = TreeInfo.getEndPos(select.selected, cu.endPositions);
        String text = source(cu, after, TreeInfo.getEndPos(select, cu.endPositions));
        int open = text.indexOf('<'), close = text.lastIndexOf('>');
        if (open < 0 || close < open) return null;
        return new Edit(after + open, after + close + 1, "", "no type arguments " + text.substring(open, close + 1) + " naming Object");
    }

    // For a method (around `path`) that has the name and the erased parameter types of a method of
    // its supertypes but does not override it, because a parameter's type is the erasure where the
    // overridden method has a type variable (`encode(Object input, ..)` for `encode(R input, ..)`
    // in a `MapEncoder.Implementation<R>`): those parameters with the overridden method's types as
    // the class inherits it. The same method seen through several supertypes counts once; two
    // methods that disagree get no edit. The erasure, and with it the method's descriptor, stays.
    static List<Edit> overriddenParameters(Types types, JCTree.JCCompilationUnit cu, TreePath path) throws Unprintable {
        while (path != null && !(path.getLeaf() instanceof JCTree.JCMethodDecl)) {
            if (path.getLeaf() instanceof ClassTree) return List.of();
            path = path.getParentPath();
        }
        if (path == null) return List.of();
        JCTree.JCMethodDecl method = (JCTree.JCMethodDecl) path.getLeaf();
        if (method.sym == null || !(method.sym.owner instanceof Symbol.ClassSymbol owner)) return List.of();
        Symbol.MethodSymbol own = method.sym;
        List<Type> overridden = null;
        for (Type supertype : types.closure(owner.type)) {
            if (supertype.tsym == owner) continue;
            for (Symbol member : supertype.tsym.members().getSymbolsByName(own.name)) {
                if (!(member instanceof Symbol.MethodSymbol other) || other.isStatic()
                        || other.params().size() != own.params().size()
                        || other.getTypeParameters().size() != own.getTypeParameters().size()
                        || own.overrides(other, owner, types, true)) continue;
                Type inherited = types.memberType(owner.type, other);
                if (!types.isSameTypes(types.erasure(inherited).getParameterTypes(),
                        types.erasure(own.type).getParameterTypes())) continue;
                // The overridden method's own type variables, as the method names them.
                List<Type> params = new ArrayList<>();
                for (Type param : inherited.getParameterTypes())
                    params.add(types.subst(param, inherited.getTypeArguments(), own.type.getTypeArguments()));
                // The same method seen through two supertypes is one; two others are ambiguous.
                if (overridden != null && !types.isSameTypes(com.sun.tools.javac.util.List.from(overridden),
                        com.sun.tools.javac.util.List.from(params))) return List.of();
                overridden = params;
            }
        }
        if (overridden == null) return List.of();
        List<Edit> edits = new ArrayList<>();
        for (int i = 0; i < method.params.size(); i++) {
            JCTree.JCVariableDecl param = method.params.get(i);
            Type wanted = overridden.get(i);
            if (param.vartype == null || param.vartype.type == null || types.isSameType(param.vartype.type, wanted)) continue;
            String written = print(cu, wanted, false);
            int start = TreeInfo.getStartPos(param.vartype), end = TreeInfo.getEndPos(param.vartype, cu.endPositions);
            edits.add(new Edit(start, end, written, "overridden parameter type " + written + ": " + param.name));
        }
        return edits;
    }

    // For `new Outer.Inner<>(..)` (around `path`), where `Inner` is an inner class of a generic
    // class `Outer` that the enclosing class is or extends: `new Inner<>(..)`, the same creation
    // with the enclosing `this` as the outer instance, which the decompiler qualified with the raw
    // outer class, which javac rejects with type arguments.
    static Edit innerOfRawQualifier(Types types, JCTree.JCCompilationUnit cu, TreePath path) {
        while (path != null && !(path.getLeaf() instanceof JCTree.JCNewClass)) {
            if (path.getLeaf() instanceof StatementTree) return null;
            path = path.getParentPath();
        }
        if (path == null) return null;
        JCTree.JCNewClass created = (JCTree.JCNewClass) path.getLeaf();
        if (created.encl != null || !(created.clazz instanceof JCTree.JCTypeApply apply)
                || !(apply.clazz instanceof JCTree.JCFieldAccess select)
                || !(TreeInfo.symbol(select) instanceof Symbol.ClassSymbol inner) || inner.isStatic()
                || !(inner.owner instanceof Symbol.ClassSymbol outer) || outer.type.getTypeArguments().isEmpty()
                || TreeInfo.symbol(select.selected) != outer) return null;
        JCTree.JCClassDecl enclosing = enclosingClass(cu, created);
        if (enclosing == null || enclosing.sym == null || !hasOuterInstance(types, enclosing.sym, outer)) return null;
        int start = TreeInfo.getStartPos(select.selected), name = TreeInfo.getEndPos(select.selected, cu.endPositions);
        String qualifier = source(cu, start, TreeInfo.getEndPos(select, cu.endPositions));
        int dot = qualifier.indexOf('.', name - start);
        if (dot < 0) return null;
        return new Edit(start, start + dot + 1, "", "inner class without the raw qualifier " + source(cu, start, start + dot + 1));
    }

    // Each type variable bounded by a member class of its own name (`Mu extends Mu`, which names
    // the variable itself), with the bound written as the member of the class
    // (`Mu extends CartesianLike.Mu`), as the original source had it.
    static List<Edit> qualifiedBounds(JCTree.JCCompilationUnit cu) {
        List<Edit> edits = new ArrayList<>();
        new TreeScanner<Void, Void>() {
            public Void visitClass(ClassTree node, Void v) {
                JCTree.JCClassDecl decl = (JCTree.JCClassDecl) node;
                for (JCTree.JCTypeParameter param : decl.typarams) {
                    for (JCTree.JCExpression bound : param.bounds) {
                        if (bound instanceof JCTree.JCIdent id && id.name.equals(param.name)
                                && decl.defs.stream().anyMatch(d -> d instanceof JCTree.JCClassDecl c && c.name.equals(param.name)))
                            edits.add(new Edit(id.getStartPosition(), id.getStartPosition(), decl.name + ".",
                                "bound " + decl.name + "." + id.name));
                    }
                }
                return super.visitClass(node, v);
            }
        }.scan(cu, null);
        return edits;
    }

    // For a type variable of the class written in a static method (at `path`), inside a cast or
    // the type arguments of a call: the code without that cast (`(T) h.apply(..)` as `h.apply(..)`)
    // or those type arguments (`Lens.<S, T, A, B>unbox(l)` as `Lens.unbox(l)`), which the decompiler
    // wrote with the names of the class's variables for the method's own. A cast to a type
    // variable makes no checkcast, and type arguments are inference only.
    static Edit withoutOuterVariable(JCTree.JCCompilationUnit cu, TreePath path) {
        for (TreePath up = path; up.getParentPath() != null; up = up.getParentPath()) {
            Tree child = up.getLeaf(), parent = up.getParentPath().getLeaf();
            if (parent instanceof JCTree.JCTypeCast cast && cast.clazz == child) {
                int start = cast.getStartPosition(), expr = cast.expr.getStartPosition();
                return new Edit(start, expr, "", "no cast " + source(cu, start, expr).strip());
            }
            if (parent instanceof JCTree.JCMethodInvocation call && call.typeargs.contains(child)
                    && call.meth instanceof JCTree.JCFieldAccess select) {
                int after = TreeInfo.getEndPos(select.selected, cu.endPositions);
                int end = TreeInfo.getEndPos(select, cu.endPositions);
                String text = source(cu, after, end);
                int open = text.indexOf('<'), close = text.lastIndexOf('>');
                if (open < 0 || close < open) return null;
                return new Edit(after + open, after + close + 1, "", "no type arguments " + text.substring(open, close + 1));
            }
            if (parent instanceof StatementTree || parent instanceof ClassTree) return null;
        }
        return null;
    }

    // For a cast whose last type argument the decompiler left empty (`(Function<Object, >) a -> x`,
    // a type it could not name), at the `>` that javac stops at: the cast to the raw type
    // (`(Function) a -> x`), which erases to the same checkcast or none.
    static Edit rawCastOfMissingArgument(JCTree.JCCompilationUnit cu, int pos) {
        String text;
        try {
            text = cu.getSourceFile().getCharContent(true).toString();
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
        if (pos < 0 || pos + 1 >= text.length() || text.charAt(pos) != '>' || text.charAt(pos + 1) != ')') return null;
        int before = pos - 1;
        while (before >= 0 && text.charAt(before) == ' ') before--;
        if (before < 0 || text.charAt(before) != ',') return null;
        int depth = 0, open = pos;
        for (; open >= 0; open--) {
            char c = text.charAt(open);
            if (c == '>') depth++;
            else if (c == '<' && --depth == 0) break;
            else if (c == '(' || c == ';' || c == '{' || c == '}') return null;
        }
        if (open < 0) return null;
        int name = open;
        while (name > 0 && (Character.isJavaIdentifierPart(text.charAt(name - 1)) || text.charAt(name - 1) == '.')) name--;
        if (name == open || name == 0 || text.charAt(name - 1) != '(') return null;
        return new Edit(open, pos + 1, "", "cast to raw " + text.substring(name, open) + " for a missing type argument");
    }

    // For an argument (at `path`) of a call `inner(..).outer(arg)` whose result is the inner
    // result's type argument (`Optional.orElse`), where `inner` is a generic method with one type
    // variable called without type arguments (`Optional.map`) and the whole call is returned or
    // initializes a variable: that type variable written out as the declared type
    // (`.<InteractionResult>map(..).orElse(InteractionResult.PASS)`), which the original had
    // where javac would infer the lambda's type instead. Type arguments are inference only.
    static Edit witness(JCTree.JCCompilationUnit cu, TreePath path) throws Unprintable {
        TreePath up = path.getParentPath();
        if (up == null || !(up.getLeaf() instanceof JCTree.JCMethodInvocation outer) || !outer.args.contains(path.getLeaf()))
            return null;
        if (!(outer.meth instanceof JCTree.JCFieldAccess select)
                || !(select.selected instanceof JCTree.JCMethodInvocation inner) || inner.typeargs.nonEmpty()
                || !(inner.meth instanceof JCTree.JCFieldAccess innerSelect)
                || !(TreeInfo.symbol(innerSelect) instanceof Symbol.MethodSymbol method)
                || method.getTypeParameters().size() != 1
                || inner.type == null || !inner.type.hasTag(TypeTag.CLASS)) return null;
        // The call on the inner result does not resolve, so its method is looked up by name.
        boolean returnsArgument = false;
        for (Symbol member : inner.type.tsym.members().getSymbolsByName(select.name)) {
            if (member instanceof Symbol.MethodSymbol result && result.params().size() == outer.args.size()
                    && result.getReturnType().hasTag(TypeTag.TYPEVAR) && result.getReturnType().tsym.owner == result.owner)
                returnsArgument = true;
        }
        if (!returnsArgument) return null;
        Type declared = declaredFor(up);
        if (declared == null || declared.isErroneous() || declared.isPrimitive()) return null;
        String written = print(cu, declared, false);
        int after = TreeInfo.getEndPos(innerSelect.selected, cu.endPositions);
        int dot = source(cu, after, TreeInfo.getEndPos(innerSelect, cu.endPositions)).indexOf('.');
        if (dot < 0) return null;
        dot += after;
        return new Edit(dot + 1, dot + 1, "<" + written + ">", "type argument <" + written + "> for " + innerSelect.name);
    }

    // The declared type that the expression at `path` is returned as or initializes, if it is.
    static Type declaredFor(TreePath path) {
        Tree leaf = path.getLeaf(), parent = path.getParentPath().getLeaf();
        if (parent instanceof JCTree.JCVariableDecl variable && variable.init == leaf && variable.vartype != null)
            return variable.vartype.type;
        if (parent instanceof JCTree.JCReturn) {
            for (TreePath up = path.getParentPath(); up != null; up = up.getParentPath()) {
                if (up.getLeaf() instanceof LambdaExpressionTree) return null;
                if (up.getLeaf() instanceof JCTree.JCMethodDecl method)
                    return method.restype == null ? null : method.restype.type;
            }
        }
        return null;
    }

    // The type of class `written` that the expression at `path` is for: the declared type of the
    // variable it initializes or of the method result it returns, where that is of the class, and
    // the raw class otherwise.
    static Type targetOf(TreePath path, Symbol.ClassSymbol written, Types types) {
        Type declared = declaredFor(path);
        return declared != null && declared.tsym == written ? declared : types.erasure(written.type);
    }

    // The detail of an error whose code is `code` or starts with it and a dot, at any depth.
    static JCDiagnostic detail(String code, JCDiagnostic diag) {
        for (Object arg : diag.getArgs()) {
            if (!(arg instanceof JCDiagnostic detail)) continue;
            if (detail.getCode().equals(code) || detail.getCode().startsWith(code + ".")) return detail;
            JCDiagnostic nested = detail(code, detail);
            if (nested != null) return nested;
        }
        return null;
    }

    // The `found cannot be converted to expected` detail of an error, also where it is the reason
    // of another (`bad type in conditional expression`).
    static JCDiagnostic inconvertible(JCDiagnostic diag) {
        for (Object arg : diag.getArgs()) {
            if (!(arg instanceof JCDiagnostic detail)) continue;
            if (detail.getCode().equals("compiler.misc.inconvertible.types") && detail.getArgs().length == 2
                    && detail.getArgs()[0] instanceof Type && detail.getArgs()[1] instanceof Type) return detail;
            JCDiagnostic nested = inconvertible(detail);
            if (nested != null) return nested;
        }
        return null;
    }

    // Whether code of class `c` has an instance of class `outer` as `this`: `c` or a class it is
    // an inner class of is or extends `outer`.
    static boolean hasOuterInstance(Types types, Symbol.ClassSymbol c, Symbol.ClassSymbol outer) {
        for (Symbol at = c; at instanceof Symbol.ClassSymbol k; at = k.owner.enclClass()) {
            if (k.isSubClass(outer, types)) return true;
            if (k.isStatic() || k.owner.enclClass() == null) return false;
        }
        return false;
    }

    // `(Expected) (Erased<?>) expr`, when the found type's erasure is a subtype of the expected
    // type's: javac then emits no checkcast, as for the original's unchecked cast. Where the
    // expected type cannot be written (it holds a captured wildcard), `(Erased) expr`, a cast to
    // the raw type, which emits no checkcast either. And `(Expected) expr` where the expected
    // type's erasure is a subtype of the found type's: the checkcast that javac emitted where the
    // original's generic call or field gave the expected type, which the decompiler wrote as the
    // erased one; in a for-each loop, the collection cast so that each element is.
    static Edit cast(Types types, JCTree.JCCompilationUnit cu, JCTree expr, Tree parent, Type found, Type expected)
            throws Unprintable {
        if (!(expr instanceof JCTree.JCExpression) || found == null || expected == null) return null;
        if (!types.isSubtype(types.erasure(found), types.erasure(expected))) {
            if (!expected.hasTag(TypeTag.CLASS) || found.hasTag(TypeTag.BOT)
                    || !types.isSubtype(types.erasure(expected), types.erasure(found))) return null;
            JCTree.JCClassDecl owner = enclosingClass(cu, expr);
            if (owner == null || owner.sym == null
                    || !checksCastTo(owner.sym.flatname.toString(), ((Symbol.ClassSymbol) expected.tsym).flatname.toString()))
                return null;
            // In a for-each loop over a raw collection, the element is what javac checks: the
            // collection as an `Iterable<Expected>` (through the raw type, unchecked) makes it
            // emit the checkcast on each element that the original's typed loop had.
            if (parent instanceof JCTree.JCEnhancedForLoop loop && loop.expr == expr) {
                if (loop.expr.type == null || !loop.expr.type.hasTag(TypeTag.CLASS)) return null;
                return wrap(cu, expr, "Iterable<" + writable(cu, expected) + ">) (Iterable", "loop over ");
            }
            return wrap(cu, expr, writable(cu, expected), "downcast to ");
        }
        String target;
        try {
            target = print(cu, expected, false);
        } catch (Unprintable e) {
            if (!expected.hasTag(TypeTag.CLASS)) throw e;
            return wrap(cu, expr, printClass(cu, (Symbol.ClassSymbol) expected.tsym), "cast to raw ");
        }
        String via = "";
        if (expected.hasTag(TypeTag.CLASS) && expected.isParameterized()) {
            if (expected.getEnclosingType().isParameterized()) throw new Unprintable();
            via = "(" + printClass(cu, (Symbol.ClassSymbol) expected.tsym) + "<"
                + expected.getTypeArguments().stream().map(t -> "?").collect(Collectors.joining(", ")) + ">) ";
        } else if (!expected.hasTag(TypeTag.TYPEVAR)) {
            return null;
        }
        int start = TreeInfo.getStartPos(expr), end = TreeInfo.getEndPos(expr, cu.endPositions);
        // In a cast to the expected type that javac rejects, only the wildcard type goes between.
        if (parent instanceof JCTree.JCTypeCast written && written.expr == expr && !via.isEmpty()
                && written.clazz.type != null && types.isSameType(written.clazz.type, expected)) {
            return new Edit(start, start, via, "cast through " + via.trim() + ": " + oneLine(source(cu, start, end)));
        }
        String source = source(cu, start, end);
        String replaced = "(" + target + ") " + via + (primary(expr) ? source : "(" + source + ")");
        return new Edit(start, end, replaced, "cast to " + target + ": " + oneLine(source));
    }

    // The innermost class declaration around `tree`.
    static JCTree.JCClassDecl enclosingClass(JCTree.JCCompilationUnit cu, JCTree tree) {
        for (TreePath up = TreePath.getPath(cu, tree); up != null; up = up.getParentPath())
            if (up.getLeaf() instanceof JCTree.JCClassDecl c) return c;
        return null;
    }

    // What the class file of a class holds that the repair checks against: the classes its
    // checkcasts cast to, and the interfaces its lambdas and method references make (the type of
    // each invokedynamic of LambdaMetafactory), as binary names.
    record ClassCode(Set<String> checkcasts, Set<String> lambdas) {}

    // The class file of class `owner` (a binary name), read once.
    static ClassCode classCode(String owner) {
        if (classes == null) return new ClassCode(Set.of(), Set.of());
        return classCodes.computeIfAbsent(owner, o -> {
            Set<String> checks = new HashSet<>(), lambdas = new HashSet<>();
            var entry = classes.getEntry(o.replace('.', '/') + ".class");
            if (entry != null) {
                byte[] bytes;
                try (InputStream in = classes.getInputStream(entry)) {
                    bytes = in.readAllBytes();
                } catch (IOException e) {
                    throw new UncheckedIOException(e);
                }
                for (var method : ClassFile.of().parse(bytes).methods())
                    for (var element : method.code().map(CodeModel::elementList).orElse(List.of())) {
                        if (element instanceof TypeCheckInstruction check && check.opcode() == Opcode.CHECKCAST)
                            checks.add(check.type().asInternalName().replace('/', '.'));
                        if (element instanceof InvokeDynamicInstruction indy
                                && indy.bootstrapMethod().owner().descriptorString()
                                    .equals("Ljava/lang/invoke/LambdaMetafactory;")) {
                            String made = indy.typeSymbol().returnType().descriptorString();
                            lambdas.add(made.substring(1, made.length() - 1).replace('/', '.'));
                        }
                    }
            }
            return new ClassCode(checks, lambdas);
        });
    }

    // Whether the class file of class `owner` (a binary name) has a checkcast to class `target`.
    static boolean checksCastTo(String owner, String target) {
        return classCode(owner).checkcasts().contains(target);
    }

    // A lambda or method reference (`expr`) for an interface that is not functional
    // (`EntityDataSerializer<T>`), where the class file makes exactly one interface for lambdas
    // that extends it (`EntityDataSerializer.ForValueType`, whose default methods leave one
    // abstract method): the original's cast to that interface, which is what javac makes the
    // lambda for. The sub-interface gets the expected type's arguments where it passes its own
    // type variables on in order, no arguments where it has none, and is raw otherwise.
    static Edit functionalCast(Types types, JCTree.JCCompilationUnit cu, JCTree expr, Type expected,
            java.util.function.Function<String, Symbol.ClassSymbol> named) throws Unprintable {
        if (!(expr instanceof JCTree.JCLambda || expr instanceof JCTree.JCMemberReference) || expected == null
                || !expected.hasTag(TypeTag.CLASS)) return null;
        JCTree.JCClassDecl owner = enclosingClass(cu, expr);
        if (owner == null || owner.sym == null) return null;
        List<Symbol.ClassSymbol> candidates = new ArrayList<>();
        for (String made : classCode(owner.sym.flatname.toString()).lambdas()) {
            Symbol.ClassSymbol sub = named.apply(made);
            if (sub != null && sub != expected.tsym && types.isSubtype(types.erasure(sub.type), types.erasure(expected)))
                candidates.add(sub);
        }
        if (candidates.size() != 1) return null;
        Symbol.ClassSymbol sub = candidates.get(0);
        String target = printClass(cu, sub);
        com.sun.tools.javac.util.List<Type> own = sub.type.getTypeArguments();
        if (!own.isEmpty()) {
            Type up = types.asSuper(sub.type, expected.tsym);
            if (up != null && types.isSameTypes(up.getTypeArguments(), own) && expected.isParameterized()) {
                List<String> arguments = new ArrayList<>();
                for (Type argument : expected.getTypeArguments()) arguments.add(print(cu, argument, false));
                target += "<" + String.join(", ", arguments) + ">";
            }
        }
        return wrap(cu, expr, target, "lambda for ");
    }

    // The expected type as source writes it, or its raw class where it cannot be written.
    static String writable(JCTree.JCCompilationUnit cu, Type expected) throws Unprintable {
        try {
            return print(cu, expected, false);
        } catch (Unprintable e) {
            return printClass(cu, (Symbol.ClassSymbol) expected.tsym);
        }
    }

    // `(target) expr`, parenthesized where it is not a primary expression.
    static Edit wrap(JCTree.JCCompilationUnit cu, JCTree expr, String target, String what) {
        int start = TreeInfo.getStartPos(expr), end = TreeInfo.getEndPos(expr, cu.endPositions);
        String source = source(cu, start, end);
        String replaced = "(" + target + ") " + (primary(expr) ? source : "(" + source + ")");
        return new Edit(start, end, replaced, what + target + ": " + oneLine(source));
    }

    static boolean primary(JCTree expr) {
        return expr instanceof JCTree.JCIdent || expr instanceof JCTree.JCFieldAccess
            || expr instanceof JCTree.JCMethodInvocation || expr instanceof JCTree.JCParens
            || expr instanceof JCTree.JCNewClass || expr instanceof JCTree.JCLiteral
            || expr instanceof JCTree.JCArrayAccess;
    }

    // `new C<>(...)` initializing a variable or returned from a method whose declared type is C<A>:
    // `new C<A>(...)`.
    // A diamond for each argument `new C(..)` of the innermost call around the error that makes a
    // generic class C raw, which leaves javac nothing to infer the call's type variables from:
    // the decompiler's way of writing the original's `new C<>(..)`.
    static List<Edit> rawArguments(JCTree.JCCompilationUnit cu, TreePath path) {
        while (path != null && !(path.getLeaf() instanceof JCTree.JCMethodInvocation)) {
            if (path.getLeaf() instanceof JCTree.JCStatement) return List.of();
            path = path.getParentPath();
        }
        if (path == null) return List.of();
        List<Edit> edits = new ArrayList<>();
        for (JCTree.JCExpression argument : ((JCTree.JCMethodInvocation) path.getLeaf()).args) {
            if (!(argument instanceof JCTree.JCNewClass created) || created.typeargs.nonEmpty()
                    || created.clazz instanceof JCTree.JCTypeApply || created.clazz.type == null
                    || created.clazz.type.tsym.type.getTypeArguments().isEmpty()) continue;
            int end = TreeInfo.getEndPos(created.clazz, cu.endPositions);
            edits.add(new Edit(end, end, "<>", "diamond in an argument: " + oneLine(source(cu,
                TreeInfo.getStartPos(created), end)) + "<>"));
        }
        return edits;
    }

    static Edit diamond(JCTree.JCCompilationUnit cu, TreePath path) throws Unprintable {
        while (path != null && !(path.getLeaf() instanceof JCTree.JCNewClass)) path = path.getParentPath();
        if (path == null) return null;
        JCTree.JCNewClass created = (JCTree.JCNewClass) path.getLeaf();
        if (!(created.clazz instanceof JCTree.JCTypeApply apply) || !apply.arguments.isEmpty()) return null;
        Tree parent = path.getParentPath().getLeaf();
        Type declared = null;
        if (parent instanceof JCTree.JCVariableDecl variable && variable.init == created && variable.vartype != null) {
            declared = variable.vartype.type;
        } else if (parent instanceof JCTree.JCReturn) {
            for (TreePath up = path; up != null; up = up.getParentPath()) {
                if (up.getLeaf() instanceof LambdaExpressionTree) return null;
                if (up.getLeaf() instanceof JCTree.JCMethodDecl method) {
                    declared = method.restype == null ? null : method.restype.type;
                    break;
                }
            }
        }
        if (declared == null || !declared.hasTag(TypeTag.CLASS) || !declared.isParameterized()) return null;
        Symbol made = TreeInfo.symbol(apply.clazz);
        if (made == null || declared.tsym != made) return null;
        StringBuilder arguments = new StringBuilder();
        for (Type argument : declared.getTypeArguments()) {
            if (argument.hasTag(TypeTag.WILDCARD)) return null;
            if (arguments.length() > 0) arguments.append(", ");
            arguments.append(print(cu, argument, false));
        }
        int end = TreeInfo.getEndPos(apply, cu.endPositions);
        return new Edit(end - 2, end, "<" + arguments + ">", "diamond: <" + arguments + ">");
    }

    // The unresolved name, used in catch blocks: renamed at every use to the parameter of the
    // catch that encloses all of its uses in the same method or lambda, if that parameter is not
    // used in its block and no other catch around a use has an unused parameter.
    static List<Edit> renameCatchVariable(JCTree.JCCompilationUnit cu, TreePath path, String name, Set<Integer> done) {
        TreePath body = path;
        while (body != null && !(body.getLeaf() instanceof MethodTree) && !(body.getLeaf() instanceof LambdaExpressionTree))
            body = body.getParentPath();
        if (body == null) return List.of();
        // Uses of the name that resolve to nothing, with the catches around each.
        List<JCTree.JCIdent> uses = new ArrayList<>();
        List<List<JCTree.JCCatch>> around = new ArrayList<>();
        new TreePathScanner<Void, Void>() {
            public Void visitIdentifier(IdentifierTree node, Void v) {
                JCTree.JCIdent ident = (JCTree.JCIdent) node;
                if (ident.name.contentEquals(name) && unresolved(ident)) {
                    uses.add(ident);
                    List<JCTree.JCCatch> catches = new ArrayList<>();
                    for (TreePath up = getCurrentPath(); up != null; up = up.getParentPath())
                        if (up.getLeaf() instanceof JCTree.JCCatch c) catches.add(c);
                    around.add(catches);
                }
                return null;
            }
        }.scan(body, null);
        if (uses.isEmpty()) return List.of();
        // Each use must have exactly one catch around it whose parameter is unused, the same one
        // for all: with two (a use in a nested catch), either could be the variable it reads.
        JCTree.JCCatch chosen = null;
        for (List<JCTree.JCCatch> catches : around) {
            List<JCTree.JCCatch> unused = catches.stream().filter(c -> !usesVariable(c)).toList();
            if (unused.size() != 1 || (chosen != null && unused.get(0) != chosen)) return List.of();
            chosen = unused.get(0);
        }
        String parameter = chosen.param.name.toString();
        if (declares(chosen.body, parameter)) return List.of(); // the parameter would be shadowed
        List<Edit> edits = new ArrayList<>();
        for (JCTree.JCIdent use : uses) {
            done.add(use.getStartPosition());
            edits.add(new Edit(use.getStartPosition(), use.getStartPosition() + name.length(), parameter,
                "renamed " + name + " to the catch parameter " + parameter));
        }
        return edits;
    }

    // Whether a variable of that name is declared inside the tree (a local, a lambda's or a nested
    // catch's parameter), which would shadow a catch parameter of the name.
    static boolean declares(JCTree tree, String name) {
        boolean[] found = {false};
        new TreeScanner<Void, Void>() {
            public Void visitVariable(VariableTree node, Void v) {
                if (node.getName().contentEquals(name)) found[0] = true;
                return super.visitVariable(node, v);
            }
        }.scan(tree, null);
        return found[0];
    }

    // A name that resolves to nothing (not a variable whose type is erroneous).
    static boolean unresolved(JCTree.JCIdent ident) {
        return ident.sym == null || ident.sym.kind == Kinds.Kind.ERR || ident.sym.kind.isResolutionError();
    }

    static boolean usesVariable(JCTree.JCCatch c) {
        Symbol parameter = c.param.sym;
        boolean[] used = {false};
        new TreeScanner<Void, Void>() {
            public Void visitIdentifier(IdentifierTree node, Void v) {
                if (((JCTree.JCIdent) node).sym == parameter) used[0] = true;
                return null;
            }
        }.scan(c.body, null);
        return used[0];
    }

    // A type as source, with each class named as the file can name it.
    static String print(JCTree.JCCompilationUnit cu, Type type, boolean argument) throws Unprintable {
        switch (type.getTag()) {
            case CLASS -> {
                if (type.isCompound() || type.isIntersection() || type.isUnion()) throw new Unprintable();
                Symbol.ClassSymbol symbol = (Symbol.ClassSymbol) type.tsym;
                String name;
                if (type.getEnclosingType().isParameterized()) {
                    name = print(cu, type.getEnclosingType(), false) + "." + symbol.name;
                } else {
                    name = printClass(cu, symbol);
                }
                if (type.getTypeArguments().isEmpty()) return name;
                StringBuilder out = new StringBuilder(name).append('<');
                for (Type t : type.getTypeArguments()) {
                    if (out.charAt(out.length() - 1) != '<') out.append(", ");
                    out.append(print(cu, t, true));
                }
                return out.append('>').toString();
            }
            case TYPEVAR -> {
                if (type instanceof Type.CapturedType captured) {
                    if (!argument) throw new Unprintable();
                    return print(cu, captured.wildcard, true);
                }
                return type.tsym.name.toString();
            }
            case WILDCARD -> {
                Type.WildcardType wildcard = (Type.WildcardType) type;
                if (!argument) throw new Unprintable();
                return switch (wildcard.kind) {
                    case UNBOUND -> "?";
                    case EXTENDS -> "? extends " + print(cu, wildcard.type, false);
                    case SUPER -> "? super " + print(cu, wildcard.type, false);
                };
            }
            case ARRAY -> {
                return print(cu, ((Type.ArrayType) type).elemtype, false) + "[]";
            }
            case BYTE, CHAR, SHORT, INT, LONG, FLOAT, DOUBLE, BOOLEAN -> {
                return type.toString();
            }
            default -> throw new Unprintable();
        }
    }

    // A class by its simple name when the file imports it or it is in the file's package or in
    // java.lang (and is not shadowed by a class declared in the file), otherwise by its
    // enclosing class or its qualified name.
    static String printClass(JCTree.JCCompilationUnit cu, Symbol.ClassSymbol symbol) throws Unprintable {
        if (symbol.isDirectlyOrIndirectlyLocal() || symbol.name.isEmpty()) {
            throw new Unprintable();
        }
        if (symbol.owner instanceof Symbol.ClassSymbol owner) {
            return printClass(cu, owner) + "." + symbol.name;
        }
        String simple = symbol.name.toString(), qualified = symbol.getQualifiedName().toString();
        for (JCTree tree : cu.defs) {
            if (tree instanceof JCTree.JCClassDecl declared && declared.name.contentEquals(simple)) {
                return declared.sym == symbol ? simple : qualified;
            }
        }
        for (ImportTree imp : cu.getImports()) {
            if (!imp.isStatic() && imp.getQualifiedIdentifier().toString().equals(qualified)) return simple;
        }
        String pkg = symbol.packge().fullname.toString();
        if (pkg.equals("java.lang") || pkg.equals(cu.getPackageName() == null ? "" : cu.getPackageName().toString())) {
            for (ImportTree imp : cu.getImports()) {
                String imported = imp.getQualifiedIdentifier().toString();
                if (!imp.isStatic() && imported.endsWith("." + simple)) return qualified;
            }
            return simple;
        }
        return qualified;
    }

    static String source(JCTree.JCCompilationUnit cu, int start, int end) {
        try {
            return cu.getSourceFile().getCharContent(true).subSequence(start, end).toString();
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    static String apply(String text, List<Edit> edits) {
        List<Edit> sorted = new ArrayList<>(edits);
        sorted.sort(Comparator.comparingInt(Edit::start).reversed());
        StringBuilder out = new StringBuilder(text);
        int limit = Integer.MAX_VALUE;
        for (Edit edit : sorted) {
            if (edit.end() > limit) continue; // overlaps an edit after it; the next round sees it again
            out.replace(edit.start(), edit.end(), edit.text());
            limit = edit.start();
        }
        return out.toString();
    }

    static int line(String text, int pos) {
        int line = 1;
        for (int i = 0; i < pos && i < text.length(); i++) if (text.charAt(i) == '\n') line++;
        return line;
    }

    static String oneLine(String s) {
        s = s.replaceAll("\\s+", " ");
        return s.length() > 80 ? s.substring(0, 77) + "..." : s;
    }
}
