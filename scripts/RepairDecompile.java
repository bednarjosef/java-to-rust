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
//   use of the name and is not used itself (the decompiler declared `e` and used `ex`), or, in
//   nested catches, to the one whose exception variable the class file reads there;
// - an unresolved variable that is assigned declared with the assigned value's type
//   (`List<PositionStep> var24;`, a temporary of a record pattern), and a local variable declared
//   again in the same scope renamed, with its uses after it;
// - before the first round, a method that the source does not declare static but its class file
//   does declared static (`static` for the `default` Procyon writes for a static interface
//   method);
// - CFR's `SwitchBootstraps.typeSwitch(..)` call replaced with the index its call site returns
//   (`x == null ? -1 : n <= 0 && x instanceof A ? 0 : ..`), the one edit that is not the class
//   file's bytecode but computes the same;
// - at an error of inference or of a call no method fits, these options, each tried when the
//   ones before did not lower the file's errors: the type arguments of the calls around the
//   error worked out from the types the expression is for, the arguments' declared types and
//   what lambdas and method references return (`MethodInfo.<Params>paramsTypedCodec()`), first
//   of the calls javac names, then of every call, then of every call with a receiver that needs
//   inference typed by what the call is for (`.<Optic<? super P, ?>>map(..).collect(..)`), with
//   an argument the parameter then does not take cast to it (`(U) this.value`), a lambda's body
//   that its function type's result does not take cast to it, and a diamond that cannot be
//   inferred written out; the decompiler's casts to a raw class made casts to a type variable of
//   that erasure; an argument holding a captured wildcard cast to its raw class; a lambda or
//   method reference cast to the interface the class file makes it for; the receiver whose
//   wildcard javac captured cast to its raw class; the cast through a wildcard type to the
//   declared type of the whole; a diamond for each raw `new C(..)` passed anywhere in the
//   statement; the method's one type variable that its code never names written for the `?` in
//   the statement's casts (the decompiler's `?` for `B`); and the statement's casts that compile
//   to nothing left out (the decompiler's guesses, often with the wrong type variables:
//   `(Either<Either<B, C>, A>)` for `A2`), those between parameterizations of one class tried
//   first, and a cast to a wider class kept where it can choose an overload or a hidden field;
// - where a type variable of the class is named in a static method, the statement's casts that
//   compile to nothing left out, tried first;
// - where the decompiler's cast to exactly the expected type is not allowed, a cast through its
//   class's wildcard type before it (`(PointFree<Function<A, C>>) (PointFree<?>) f`);
// - a parameter whose type the decompiler erased, in a method that then overrides nothing and
//   leaves its class abstract, written with the type of the method of a supertype that has the
//   same erasure (`encode(R input, ..)` for `encode(Object input, ..)`);
// - in an argument, or in an expression whose type variables javac cannot infer, a call's type
//   arguments that name `Object` (`Either.<Object, R>unbox(e)`, types the decompiler could not
//   name) left out, and a cast to a type variable erased to `Object` that is a lambda's whole
//   body (`a -> (T) f.apply(a)`) left out (no checkcast either way);
// - `new Outer.Inner<>(..)` of an inner class of a generic class that the enclosing class is or
//   extends, which javac rejects with the raw qualifier, written `new Inner<>(..)`;
// - a method reference on a raw `new C(..)` of a class with one type variable, or on a call that
//   returns it (`Util.make(new Int2ObjectOpenHashMap(), m -> ..)::get`), whose method returns that
//   variable where the function type wants `R`: `new C<R>(..)`, the type the original's diamond
//   took from its target.
//
// Each round takes, for every error, its first option not tried before. A file whose errors
// outside imports and annotations do not go down after a round gets its previous text back; an
// option tried alone is then not tried again, and a file that had several gets one per round
// from then on. Each edit is written to LOG.
import com.sun.source.tree.*;
import com.sun.source.util.*;
import com.sun.tools.javac.api.ClientCodeWrapper;
import com.sun.tools.javac.api.JavacTaskImpl;
import com.sun.tools.javac.code.Kinds;
import com.sun.tools.javac.code.Symbol;
import com.sun.tools.javac.code.Symtab;
import com.sun.tools.javac.code.Type;
import com.sun.tools.javac.code.TypeTag;
import com.sun.tools.javac.code.Types;
import com.sun.tools.javac.tree.JCTree;
import com.sun.tools.javac.tree.TreeInfo;
import com.sun.tools.javac.util.Context;
import com.sun.tools.javac.util.JCDiagnostic;
import com.sun.tools.javac.util.Name;
import java.io.*;
import java.lang.classfile.ClassFile;
import java.lang.classfile.CodeModel;
import java.lang.classfile.Opcode;
import java.lang.classfile.Instruction;
import java.lang.classfile.TypeKind;
import java.lang.classfile.instruction.ConstantInstruction;
import java.lang.classfile.instruction.InvokeDynamicInstruction;
import java.lang.classfile.instruction.LabelTarget;
import java.lang.classfile.instruction.LoadInstruction;
import java.lang.classfile.instruction.StoreInstruction;
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
        Map<Path, List<Error>> errors = new TreeMap<>();
        boolean[] declared = {false};
        compile(all, (task, units) -> declared[0] = staticMethods(task, units), errors);
        report("before", errors);
        // Methods made static as their class files have them change other files' errors.
        if (declared[0]) {
            errors.clear();
            errors.putAll(errorsOf(all));
        }
        Set<Path> pending = new TreeSet<>(errors.keySet());
        // The options that did not lower their file's errors, which are not tried again, and the
        // files whose options are tried one at a time, since several together did not.
        Map<Path, Set<List<String>>> failed = new HashMap<>();
        Set<Path> alone = new HashSet<>();
        for (int round = 1; round <= rounds && !pending.isEmpty(); round++) {
            Map<Path, String> before = new HashMap<>();
            Map<Path, Long> counted = new HashMap<>();
            Map<Path, List<Edit>> edits = new TreeMap<>();
            Map<Path, List<List<String>>> tried = new HashMap<>();
            compile(List.copyOf(pending), (task, units) -> {
                for (CompilationUnitTree unit : units) {
                    Path file = Path.of(unit.getSourceFile().toUri());
                    List<Error> found = errors.getOrDefault(file, List.of());
                    Set<List<String>> notAgain = failed.getOrDefault(file, Set.of());
                    // For each error, its first option not tried before.
                    List<List<Edit>> chosen = new ArrayList<>();
                    List<List<String>> keys = new ArrayList<>();
                    for (List<List<Edit>> options : repairs(task, unit, found)) {
                        for (List<Edit> option : options) {
                            List<String> key = key(unit, option);
                            if (notAgain.contains(key)) continue;
                            if (!keys.contains(key)) {
                                chosen.add(option);
                                keys.add(key);
                            }
                            break;
                        }
                    }
                    if (alone.contains(file) && chosen.size() > 1) {
                        chosen = chosen.subList(0, 1);
                        keys = keys.subList(0, 1);
                    }
                    List<Edit> fileEdits = chosen.stream().flatMap(List::stream).distinct().toList();
                    if (!fileEdits.isEmpty()) {
                        edits.put(file, fileEdits);
                        tried.put(file, keys);
                    }
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
                    // One option is not tried again; of several, each is tried alone next.
                    if (tried.get(file).size() == 1) failed.computeIfAbsent(file, f -> new HashSet<>()).add(tried.get(file).get(0));
                    else alone.add(file);
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

    // What identifies an option across rounds, when other edits have moved its text: each edit's
    // text before and after.
    static List<String> key(CompilationUnitTree unit, List<Edit> option) {
        List<String> key = new ArrayList<>();
        for (Edit edit : option)
            key.add(source((JCTree.JCCompilationUnit) unit, edit.start(), edit.end()) + "\u0000" + edit.text() + "\u0000" + edit.what());
        return key;
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

    // The options for each of one file's errors: the edits of each, in the order they are tried.
    static List<List<List<Edit>>> repairs(JavacTask task, CompilationUnitTree unit, List<Error> errors) {
        Context context = ((JavacTaskImpl) task).getContext();
        Types types = Types.instance(context);
        var elements = com.sun.tools.javac.model.JavacElements.instance(context);
        java.util.function.Function<String, Symbol.ClassSymbol> named =
            binary -> (Symbol.ClassSymbol) elements.getTypeElement(binary.replace('$', '.'));
        JCTree.JCCompilationUnit cu = (JCTree.JCCompilationUnit) unit;
        Symtab syms = Symtab.instance(context);
        List<List<List<Edit>>> all = new ArrayList<>();
        Set<Integer> renamed = new HashSet<>();
        for (Error error : errors) {
            if (error.ignored()) continue;
            JCDiagnostic diag = error.diag();
            // The first option's edits, and the options after it.
            List<Edit> edits = new ArrayList<>();
            List<List<Edit>> more = new ArrayList<>(), early = new ArrayList<>();
            try {
                options(types, cu, unit, diag, named, renamed, edits, more, early);
            } catch (Unprintable e) {
                // a type that source cannot name (a captured or intersection type, a local class)
            }
            try {
                laterOptions(types, syms, cu, unit, diag, named, more, early);
            } catch (Unprintable e) {
                // a type that source cannot name (a captured or intersection type, a local class)
            }
            List<List<Edit>> options = new ArrayList<>();
            for (List<Edit> option : early) if (!option.isEmpty() && !options.contains(option)) options.add(option);
            if (!edits.isEmpty()) options.add(edits.stream().distinct().toList());
            for (List<Edit> option : more) if (!option.isEmpty() && !options.contains(option)) options.add(option);
            if (!options.isEmpty()) all.add(options);
        }
        return all;
    }

    // The options for an error of inference or of a call no method fits. Before the first: the
    // decompiler's casts between parameterizations of one class in the statement left out. After
    // it: the type arguments worked out around the error (rewriting those the decompiler wrote
    // that disagree), first of the calls whose type variables javac names, then of every call,
    // then of every call with a receiver that needs inference given its type by what the call is
    // for; the decompiler's casts to a raw type made casts to a type variable; an argument holding
    // a captured wildcard cast to its raw class; a lambda or method reference cast to the
    // interface the class file makes it for; the receiver whose wildcard javac captured cast to
    // its raw class; the cast through a wildcard type to the declared type of the whole; a
    // diamond for each raw `new C(..)` passed anywhere in the statement; the method's one type
    // variable that its code never names written for the `?` in the statement's casts; and every
    // cast in the statement that compiles to nothing left out.
    static void laterOptions(Types types, Symtab syms, JCTree.JCCompilationUnit cu, CompilationUnitTree unit,
            JCDiagnostic diag, java.util.function.Function<String, Symbol.ClassSymbol> named, List<List<Edit>> more,
            List<List<Edit>> early) throws Unprintable {
        String code = diag.getCode();
        if (!code.equals("compiler.err.prob.found.req") && !code.startsWith("compiler.err.cant.apply.symbol")
                && !code.startsWith("compiler.err.cant.apply.diamond")) return;
        JCTree tree = diag.getDiagnosticPosition() == null ? null : diag.getDiagnosticPosition().getTree();
        if (tree == null) return;
        TreePath path = pathTo(unit, tree);
        if (path == null) return;
        // Each option on its own: a type one of them cannot write leaves the others.
        attempt(more, () -> Witnesses.around(types, syms, cu, path, uninferred(diag)));
        attempt(more, () -> Witnesses.around(types, syms, cu, path, null));
        attempt(more, () -> Witnesses.around(types, syms, cu, path, null, true));
        try {
            more.addAll(typeVariableCasts(types, cu, path));
        } catch (RuntimeException e) {
            // javac's attribution of erroneous code holds what the repair does not expect
        }
        attempt(more, () -> rawCapturedArguments(cu, path));
        attempt(more, () -> functionalArguments(types, cu, path, named));
        JCDiagnostic detail = inconvertible(diag);
        if (detail != null && detail.getArgs()[1] instanceof Type.CapturedType)
            attempt(more, () -> optional(rawReceiver(cu, path)));
        attempt(more, () -> optional(declaredCast(types, cu, path)));
        attempt(more, () -> diamondsAround(cu, path));
        attempt(more, () -> unusedVariableWildcards(cu, path));
        attempt(early, () -> noOpCasts(types, cu, path, true));
        attempt(more, () -> noOpCasts(types, cu, path, false));
    }

    interface Option {
        List<Edit> edits() throws Unprintable;
    }

    // The option's edits added to `options`, unless it has none or cannot be written.
    static void attempt(List<List<Edit>> options, Option option) {
        try {
            List<Edit> edits = option.edits();
            if (!edits.isEmpty()) options.add(edits);
        } catch (Unprintable | RuntimeException e) {
            // a type that source cannot name, or attribution the repair does not expect
        }
    }

    static List<Edit> optional(Edit edit) {
        return edit == null ? List.of() : List.of(edit);
    }

    static void options(Types types, JCTree.JCCompilationUnit cu, CompilationUnitTree unit, JCDiagnostic diag,
            java.util.function.Function<String, Symbol.ClassSymbol> named, Set<Integer> renamed, List<Edit> edits,
            List<List<Edit>> more, List<List<Edit>> early) throws Unprintable {
            if (diag.getCode().equals("compiler.err.illegal.start.of.type")) {
                Edit edit = rawCastOfMissingArgument(cu, (int) diag.getPosition());
                if (edit != null) edits.add(edit);
                return;
            }
            if (diag.getCode().equals("compiler.err.cyclic.inheritance")) {
                edits.addAll(qualifiedBounds(cu));
                return;
            }
            JCTree tree = diag.getDiagnosticPosition() == null ? null : diag.getDiagnosticPosition().getTree();
            if (tree == null) return;
            TreePath path = TreePath.getPath(unit, tree);
            if (path == null) return;
            Edit typeSwitch = typeSwitch(cu, path);
            if (typeSwitch != null) {
                edits.add(typeSwitch);
                return;
            }
            {
                switch (diag.getCode()) {
                    case "compiler.err.prob.found.req" -> {
                        JCDiagnostic detail = inconvertible(diag);
                        List<Edit> diamonds = rawArguments(cu, path);
                        List<Edit> typed = typedRawQualifier(types, cu, path, detail);
                        JCDiagnostic notFunctional = detail("compiler.misc.not.a.functional.intf", diag);
                        if (!typed.isEmpty()) {
                            edits.addAll(typed);
                        } else if (notFunctional != null && notFunctional.getArgs()[0] instanceof Symbol.ClassSymbol written) {
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
                            // The cast is of the expression the error is at only where the found
                            // type is that expression's (or its elements', looped over), not an
                            // argument's inside it.
                            boolean own = path.getParentPath().getLeaf() instanceof JCTree.JCEnhancedForLoop loop && loop.expr == tree
                                || (tree.type != null && !tree.type.isErroneous()
                                    ? types.isSameType(types.erasure(tree.type), types.erasure(found)) : !inference(diag));
                            if (edit == null && own) edit = cast(types, cu, tree, path.getParentPath().getLeaf(), found, expected);
                            if (edit != null) edits.add(edit);
                            // Where the expected type holds a captured wildcard, which a cast
                            // through the wildcard type does not reach: the cast to the raw class.
                            if (own && tree instanceof JCTree.JCExpression && expected.hasTag(TypeTag.CLASS)
                                    && expected.isParameterized() && capturing(expected))
                                more.add(List.of(wrap(cu, tree, printClass(cu, (Symbol.ClassSymbol) expected.tsym), "cast to raw ")));
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
                        if (diag.getArgs()[0] == Kinds.KindName.TYPEVAR) early.add(noOpCasts(types, cu, path, false));
                    }
                    case "compiler.err.cant.apply.diamond.1" -> {
                        Edit edit = diamond(cu, path);
                        if (edit != null) edits.add(edit);
                    }
                    case "compiler.err.cant.resolve.location" -> {
                        if (diag.getArgs()[0] == Kinds.KindName.VAR && renamed.add(tree.getStartPosition())) {
                            String name = diag.getArgs()[1].toString();
                            List<Edit> renames = renameCatchVariable(cu, path, name, renamed);
                            if (renames.isEmpty()) {
                                Edit declared = declareAssigned(types, cu, path, name, renamed);
                                if (declared != null) edits.add(declared);
                            }
                            edits.addAll(renames);
                        }
                    }
                    case "compiler.err.already.defined" -> {
                        if (tree instanceof JCTree.JCVariableDecl variable) edits.addAll(renameRedeclared(cu, path, variable));
                    }
                    default -> {}
                }
            }
    }

    static class Unprintable extends Exception {}

    // Type arguments that javac could not infer in decompiled code, worked out from what the
    // original source must have given javac: the type an expression is for (the variable it
    // initializes, the result it returns, the parameter it is passed to), the declared types of
    // the arguments, and what a lambda or a method reference returns, through chains of calls
    // (`a.b(..).c(..)`, where the type `c` is for tells the type `a.b(..)` must have). The
    // decompiler writes no type arguments where javac inferred them and the original source had
    // none, but a call whose arguments the decompiler changed (a raw `new C(..)`, a lambda whose
    // parameter type is gone) can no longer be inferred. Writing the type arguments out is
    // inference only: the call, its descriptor and its result's erasure stay.
    static final class Witnesses {
        final Types types;
        final Symtab syms;
        final JCTree.JCCompilationUnit cu;
        // The methods whose type arguments are written: those whose type variables javac said it
        // could not infer, or every method when null. Type arguments the decompiler wrote that
        // disagree with the worked-out ones are rewritten either way.
        final Set<Symbol> named;
        // Whether a receiver whose type needs inference gets it from what the call is for, rather
        // than from below: `s.map(e -> e.optic()).collect(toList())` assigned to a
        // `List<Optic<? super P, ?>>` maps to that type, the lambda's own being narrower.
        boolean fromTarget;
        final Set<Symbol> scope = new HashSet<>();
        final Map<Symbol, Type> locals = new HashMap<>();
        final Map<Integer, Edit> edits = new TreeMap<>();
        int depth;

        Witnesses(Types types, Symtab syms, JCTree.JCCompilationUnit cu, Set<Symbol> named) {
            this.types = types;
            this.syms = syms;
            this.cu = cu;
            this.named = named;
        }

        // The edits for the expression around the error at `path`: from the outermost expression
        // that the error's is part of, through calls, lambdas and conditionals.
        static List<Edit> around(Types types, Symtab syms, JCTree.JCCompilationUnit cu, TreePath path, Set<Symbol> named) {
            return around(types, syms, cu, path, named, false);
        }

        static List<Edit> around(Types types, Symtab syms, JCTree.JCCompilationUnit cu, TreePath path, Set<Symbol> named,
                boolean fromTarget) {
            TreePath top = path;
            Type target = null;
            while (top.getParentPath() != null) {
                Tree leaf = top.getLeaf(), parent = top.getParentPath().getLeaf();
                if (parent instanceof JCTree.JCMethodInvocation || parent instanceof JCTree.JCFieldAccess
                        || parent instanceof JCTree.JCParens || parent instanceof JCTree.JCConditional
                        || parent instanceof JCTree.JCLambda || parent instanceof JCTree.JCTypeCast
                        || parent instanceof JCTree.JCNewClass && ((JCTree.JCNewClass) parent).def == null
                        || parent instanceof JCTree.JCMemberReference) {
                    top = top.getParentPath();
                    continue;
                }
                if (parent instanceof JCTree.JCReturn) {
                    TreePath up = top.getParentPath();
                    while (up != null && !(up.getLeaf() instanceof JCTree.JCLambda || up.getLeaf() instanceof JCTree.JCMethodDecl
                            || up.getLeaf() instanceof JCTree.JCClassDecl))
                        up = up.getParentPath();
                    if (up != null && up.getLeaf() instanceof JCTree.JCLambda) {
                        top = up;
                        continue;
                    }
                }
                if (leaf instanceof JCTree.JCExpression expr) target = declaredFor(top);
                if (parent instanceof JCTree.JCAssign assign && assign.rhs == leaf && assign.lhs.type != null)
                    target = assign.lhs.type;
                break;
            }
            if (!(top.getLeaf() instanceof JCTree.JCExpression expr)) return List.of();
            Witnesses w = new Witnesses(types, syms, cu, named);
            w.fromTarget = fromTarget;
            w.scopeOf(top);
            try {
                w.down(expr, target);
            } catch (RuntimeException e) {
                // javac's attribution of erroneous code holds what the solving does not expect
                return List.of();
            }
            return List.copyOf(w.edits.values());
        }

        // The type variables that code at `path` can name: those of the methods around it, and
        // of the classes around it up to a static member.
        void scopeOf(TreePath path) {
            boolean statics = false;
            for (TreePath up = path; up != null; up = up.getParentPath()) {
                if (up.getLeaf() instanceof JCTree.JCMethodDecl method && method.sym != null) {
                    for (Symbol.TypeVariableSymbol v : method.sym.getTypeParameters()) scope.add(v);
                    if (method.sym.isStatic()) statics = true;
                } else if (up.getLeaf() instanceof JCTree.JCVariableDecl field && field.sym != null
                        && field.sym.owner instanceof Symbol.ClassSymbol && field.sym.isStatic()) {
                    statics = true;
                } else if (up.getLeaf() instanceof JCTree.JCClassDecl c && c.sym != null) {
                    if (!statics) for (Symbol.TypeVariableSymbol v : c.sym.getTypeParameters()) scope.add(v);
                    if (c.sym.isStatic() || c.sym.owner instanceof Symbol.PackageSymbol) statics = true;
                }
            }
        }

        // Whether `t` can be written here as a type argument: no captured type, no type variable
        // out of scope, nothing raw or erroneous.
        boolean writable(Type t) {
            if (t == null || t.isErroneous()) return false;
            switch (t.getTag()) {
                case CLASS -> {
                    if (t.isCompound() || t.isIntersection() || t.isUnion() || t.isRaw()) return false;
                    if (t.getEnclosingType().hasTag(TypeTag.CLASS) && !writable(t.getEnclosingType())) return false;
                    for (Type a : t.getTypeArguments()) {
                        if (a.hasTag(TypeTag.WILDCARD)) {
                            Type.WildcardType w = (Type.WildcardType) a;
                            if (w.kind != com.sun.tools.javac.code.BoundKind.UNBOUND && !writable(w.type)) return false;
                        } else if (!writable(a)) return false;
                    }
                    return true;
                }
                case TYPEVAR -> {
                    return !(t instanceof Type.CapturedType) && scope.contains(t.tsym);
                }
                case ARRAY -> {
                    return writable(((Type.ArrayType) t).elemtype);
                }
                default -> {
                    return t.isPrimitive();
                }
            }
        }

        // How a type pattern stands to the actual type it is matched with.
        enum Way { SAME, BELOW, ABOVE }

        // What the matches tell of each unknown type variable: the types it must be, be a
        // subtype of, or be a supertype of.
        final class Constraints {
            final Set<Type> unknown;
            final Map<Type, List<Type>> exact = new HashMap<>(), upper = new HashMap<>(), lower = new HashMap<>();

            Constraints(Set<Type> unknown) {
                this.unknown = unknown;
            }

            void add(Type v, Type t, Way way) {
                if (t.isPrimitive()) t = types.boxedClass(t).type;
                if (!writable(t)) return;
                (way == Way.SAME ? exact : way == Way.BELOW ? upper : lower).computeIfAbsent(v, x -> new ArrayList<>()).add(t);
            }

            // Each variable's type: the one it must be; else the most specific of the types it
            // must be below (what a type argument written for it can be, the arguments then
            // converting to it); else the most general of those it must be above. A variable
            // whose types disagree stays unknown.
            Map<Type, Type> resolve() {
                Map<Type, Type> bound = new LinkedHashMap<>();
                for (Type v : unknown) {
                    Type t;
                    if (exact.containsKey(v)) {
                        t = one(exact.get(v), null);
                        if (t == null) continue;
                    } else if (upper.containsKey(v)) {
                        t = one(upper.get(v), Way.BELOW);
                    } else if (lower.containsKey(v)) {
                        t = one(lower.get(v), Way.ABOVE);
                    } else {
                        continue;
                    }
                    if (t != null) bound.put(v, t);
                }
                return bound;
            }

            // The one type of `ts`, or with `way`, the one of them below (or above) all others.
            Type one(List<Type> ts, Way way) {
                for (Type t : ts) {
                    boolean fits = true;
                    for (Type other : ts) {
                        if (types.isSameType(t, other)) continue;
                        if (way == null || !(way == Way.BELOW ? types.isSubtype(t, other) : types.isSubtype(other, t))) {
                            fits = false;
                            break;
                        }
                    }
                    if (fits) return t;
                }
                return null;
            }

            // Matches `pattern`, which holds unknown type variables, with `actual`.
            void unify(Type pattern, Type actual, Way way) {
                if (pattern == null || actual == null || actual.isErroneous() || actual.hasTag(TypeTag.BOT)) return;
                if (pattern.hasTag(TypeTag.TYPEVAR) && unknown.contains(pattern)) {
                    if (!actual.hasTag(TypeTag.WILDCARD)) add(pattern, actual, way);
                    return;
                }
                if (pattern.hasTag(TypeTag.ARRAY) && actual.hasTag(TypeTag.ARRAY)) {
                    unify(((Type.ArrayType) pattern).elemtype, ((Type.ArrayType) actual).elemtype, way);
                    return;
                }
                if (!pattern.hasTag(TypeTag.CLASS) || !actual.hasTag(TypeTag.CLASS) || actual.isRaw()) return;
                Type p = pattern, a = actual;
                if (p.tsym != a.tsym) {
                    if (way == Way.SAME) return;
                    if (way == Way.BELOW) p = types.asSuper(p, a.tsym);
                    else a = types.asSuper(a, p.tsym);
                    if (p == null || a == null || p.tsym != a.tsym || a.isRaw()) return;
                }
                var ps = p.getTypeArguments();
                var as = a.getTypeArguments();
                if (ps.size() != as.size()) return;
                for (int i = 0; i < ps.size(); i++) argument(ps.get(i), as.get(i));
            }

            // Matches a type argument of a pattern with the actual one in the same place.
            void argument(Type p, Type a) {
                var UNBOUND = com.sun.tools.javac.code.BoundKind.UNBOUND;
                var EXTENDS = com.sun.tools.javac.code.BoundKind.EXTENDS;
                if (p.hasTag(TypeTag.WILDCARD)) {
                    Type.WildcardType w = (Type.WildcardType) p;
                    if (w.kind == UNBOUND) return;
                    Type.WildcardType aw = a.hasTag(TypeTag.WILDCARD) ? (Type.WildcardType) a : null;
                    if (w.kind == EXTENDS) {
                        // `? extends X` holds a type, a `? extends` of one, and `?` (any type is one of Object).
                        if (aw == null) unify(w.type, a, Way.ABOVE);
                        else if (aw.kind == EXTENDS) unify(w.type, aw.type, Way.ABOVE);
                        else if (aw.kind == UNBOUND) unify(w.type, syms.objectType, Way.ABOVE);
                    } else {
                        if (aw == null) unify(w.type, a, Way.BELOW);
                        else if (aw.kind != EXTENDS && aw.kind != UNBOUND) unify(w.type, aw.type, Way.BELOW);
                    }
                    return;
                }
                if (a.hasTag(TypeTag.WILDCARD)) {
                    Type.WildcardType aw = (Type.WildcardType) a;
                    if (aw.kind == UNBOUND) return;
                    unify(p, aw.type, aw.kind == EXTENDS ? Way.BELOW : Way.ABOVE);
                    return;
                }
                unify(p, a, Way.SAME);
            }
        }

        static com.sun.tools.javac.util.List<Type> list(List<Type> types) {
            return com.sun.tools.javac.util.List.from(types);
        }

        Type subst(Type t, Map<Type, Type> bound) {
            if (t == null || bound.isEmpty()) return t;
            List<Type> from = new ArrayList<>(bound.keySet()), to = new ArrayList<>(bound.values());
            return types.subst(t, list(from), list(to));
        }

        // Whether `t` mentions one of `unknown`.
        static boolean mentions(Type t, Collection<Type> unknown) {
            return t != null && !unknown.isEmpty() && t.containsAny(list(new ArrayList<>(unknown)));
        }

        // The type of `expr` that needs no inference: a variable's or a field's declared type (a
        // lambda parameter's as worked out), a literal's or a cast's, `this`, and a call of a method
        // that is not generic on such a receiver. Null where it needs inference.
        Type known(JCTree.JCExpression expr) {
            if (expr == null) return null;
            Type t = null;
            switch (expr) {
                case JCTree.JCParens parens -> t = known(parens.expr);
                case JCTree.JCLiteral literal -> t = literal.type;
                case JCTree.JCTypeCast cast -> t = cast.clazz.type;
                case JCTree.JCIdent ident -> {
                    Symbol s = ident.sym;
                    if (s instanceof Symbol.VarSymbol var) t = locals.containsKey(var) ? locals.get(var) : var.type;
                    else if (s != null && s.name == s.name.table.names._this) t = ident.type;
                }
                case JCTree.JCFieldAccess access -> {
                    Symbol s = access.sym;
                    if (s instanceof Symbol.VarSymbol var) {
                        if (var.isStatic()) t = var.type;
                        else {
                            Type site = known(access.selected);
                            if (site != null && site.hasTag(TypeTag.CLASS)) t = types.memberType(site, var);
                        }
                    }
                }
                case JCTree.JCMethodInvocation call -> {
                    Symbol s = TreeInfo.symbol(call.meth);
                    if (s instanceof Symbol.MethodSymbol m && m.getTypeParameters().isEmpty() && call.typeargs.isEmpty()) {
                        Type site = siteOf(call, m);
                        if (site != null || m.isStatic()) t = (site == null ? m.type : types.memberType(site, m)).getReturnType();
                        if (t != null && site == null && mentions(t, m.owner.type.getTypeArguments())) t = null;
                    }
                }
                case JCTree.JCNewClass created -> {
                    // The type arguments of a generic class are the decompiler's, which may be wrong.
                    if (created.def == null && created.clazz.type != null && created.clazz.type.tsym.type.getTypeArguments().isEmpty())
                        t = created.clazz.type;
                }
                default -> {}
            }
            return t == null || t.isErroneous() || t.hasTag(TypeTag.METHOD) || t.hasTag(TypeTag.FORALL) ? null : t;
        }

        // The type of `expr` as javac gives it on its own: the type that needs no inference, or
        // that of a call of a method that is not generic, which javac does not infer from where
        // the call is.
        Type standalone(JCTree.JCExpression expr) {
            Type t = known(expr);
            if (t == null && expr instanceof JCTree.JCMethodInvocation call
                    && TreeInfo.symbol(call.meth) instanceof Symbol.MethodSymbol m && m.getTypeParameters().isEmpty()
                    && call.type != null && !call.type.isErroneous() && call.type.hasTag(TypeTag.CLASS))
                t = call.type;
            return t;
        }

        // The type of the receiver of `call` of instance method `m` that needs no inference: the
        // enclosing class's for an unqualified call. Null for a static method or where unknown.
        Type siteOf(JCTree.JCMethodInvocation call, Symbol.MethodSymbol m) {
            if (m.isStatic()) return null;
            if (call.meth instanceof JCTree.JCFieldAccess access) {
                Type t = known(access.selected);
                return t == null || !t.hasTag(TypeTag.CLASS) ? null : types.asSuper(t, m.owner);
            }
            JCTree.JCClassDecl c = enclosingClass(cu, call);
            return c == null || c.sym == null ? null : types.asOuterSuper(c.sym.type, m.owner);
        }

        // Works the type arguments out for `expr` as an expression of type `target` (null, or
        // holding type variables still unknown, where that is not known), adding the edits that
        // write them. Returns the type `expr` then has, where that is known.
        Type down(JCTree.JCExpression expr, Type target) {
            if (expr == null || ++depth > 100000) return null;
            switch (expr) {
                case JCTree.JCParens parens -> {
                    return down(parens.expr, target);
                }
                case JCTree.JCConditional conditional -> {
                    Type a = down(conditional.truepart, target), b = down(conditional.falsepart, target);
                    return a != null && b != null && types.isSameType(a, b) ? a : null;
                }
                case JCTree.JCTypeCast cast -> {
                    // A cast through a wildcard type (`(Codec<X>) (Codec<?>) e`) passes the type on.
                    Type to = cast.clazz.type;
                    boolean through = to != null && to.hasTag(TypeTag.CLASS) && !to.getTypeArguments().isEmpty()
                        && to.getTypeArguments().stream().allMatch(a -> a.hasTag(TypeTag.WILDCARD)
                            && ((Type.WildcardType) a).kind == com.sun.tools.javac.code.BoundKind.UNBOUND);
                    down(cast.expr, through ? target : to);
                    return known(cast);
                }
                case JCTree.JCMethodInvocation call -> {
                    return call(call, target);
                }
                case JCTree.JCNewClass created -> {
                    return created(created, target);
                }
                case JCTree.JCLambda lambda -> {
                    lambda(lambda, target);
                    return null;
                }
                default -> {
                    return known(expr);
                }
            }
        }

        // The function type of `target` with its parameter and result types, or null.
        Type descriptor(Type target) {
            if (target == null || !target.hasTag(TypeTag.CLASS) || !target.tsym.isInterface()) return null;
            try {
                return types.findDescriptorType(target);
            } catch (RuntimeException e) {
                return null;
            }
        }

        // The lambda's parameters given the types of `target`'s function type, where those are
        // known, and its result worked out for that type's result.
        void lambda(JCTree.JCLambda lambda, Type target) {
            Type descriptor = descriptor(target);
            if (descriptor == null) return;
            var params = descriptor.getParameterTypes();
            if (params.size() == lambda.params.size())
                for (int i = 0; i < params.size(); i++)
                    if (lambda.params.get(i).vartype == null || lambda.paramKind == JCTree.JCLambda.ParameterKind.IMPLICIT)
                        if (writable(params.get(i))) locals.put(lambda.params.get(i).sym, params.get(i));
            Type result = descriptor.getReturnType();
            if (result.hasTag(TypeTag.VOID)) result = null;
            for (JCTree.JCExpression returned : results(lambda)) down(returned, result);
        }

        // The expressions a lambda returns.
        static List<JCTree.JCExpression> results(JCTree.JCLambda lambda) {
            if (lambda.body instanceof JCTree.JCExpression body) return List.of(body);
            List<JCTree.JCExpression> found = new ArrayList<>();
            new TreeScanner<Void, Void>() {
                public Void visitReturn(ReturnTree node, Void v) {
                    if (node.getExpression() != null) found.add((JCTree.JCExpression) node.getExpression());
                    return null;
                }
                public Void visitLambdaExpression(LambdaExpressionTree node, Void v) { return null; }
                public Void visitClass(ClassTree node, Void v) { return null; }
            }.scan(lambda.body, null);
            return found;
        }

        // What a lambda or a method reference returns given the parameter types of
        // `descriptor`, where that needs no inference.
        Type resultOf(JCTree.JCExpression function, Type descriptor) {
            var params = descriptor.getParameterTypes();
            if (function instanceof JCTree.JCLambda lambda) {
                if (!(lambda.body instanceof JCTree.JCExpression body) || params.size() != lambda.params.size()) return null;
                for (int i = 0; i < params.size(); i++) {
                    if (!writable(params.get(i))) return null;
                    if (lambda.paramKind == JCTree.JCLambda.ParameterKind.IMPLICIT) locals.put(lambda.params.get(i).sym, params.get(i));
                }
                Type t = known(body);
                if (t == null && body instanceof JCTree.JCMethodInvocation) {
                    // A generic call whose type arguments its arguments give.
                    Map<Integer, Edit> kept = new TreeMap<>(edits);
                    t = down(body, null);
                    edits.clear();
                    edits.putAll(kept);
                }
                return t;
            }
            if (!(function instanceof JCTree.JCMemberReference ref)) return null;
            Symbol.MethodSymbol m = ref.sym instanceof Symbol.MethodSymbol s ? s : referenced(ref, params.size());
            if (m == null) return null;
            if (m.isConstructor()) {
                Type made = ref.expr.type;
                return made != null && made.hasTag(TypeTag.CLASS) && (made.tsym.type.getTypeArguments().isEmpty()
                    || ref.expr instanceof JCTree.JCTypeApply) ? made : null;
            }
            Type site = null;
            // `C::m` takes its receiver as the first parameter; `e::m` has `e`'s.
            boolean unbound = !m.isStatic() && (ref.expr instanceof JCTree.JCIdent id && id.sym instanceof Symbol.ClassSymbol
                || ref.expr instanceof JCTree.JCFieldAccess fa && fa.sym instanceof Symbol.ClassSymbol
                || ref.expr instanceof JCTree.JCTypeApply);
            if (!m.isStatic()) {
                site = unbound ? (params.isEmpty() ? null : params.get(0)) : known(ref.expr);
                if (site == null || !site.hasTag(TypeTag.CLASS)) return null;
                site = types.asSuper(site, m.owner);
                if (site == null) return null;
            }
            Type type = site == null ? m.type : types.memberType(site, m);
            Type result = type.getReturnType();
            if (site == null && !m.isStatic()) return null;
            if (result == null || result.hasTag(TypeTag.VOID)) return null;
            List<Type> variables = type.getTypeArguments();
            if (variables.isEmpty()) return result;
            // A generic method's type variables, from the function type's parameters it takes;
            // those still unknown become wildcards in the result, a type it is a subtype of.
            Constraints k = new Constraints(new LinkedHashSet<>(variables));
            var own = type.getParameterTypes();
            int skip = unbound ? 1 : 0;
            if (own.size() + skip == params.size())
                for (int i = 0; i < own.size(); i++) k.unify(own.get(i), params.get(i + skip), Way.ABOVE);
            return wildcarded(subst(result, k.resolve()), variables);
        }

        // The method `ref` refers to where javac resolved none: the one of its name in the
        // qualifier's class that takes `count` parameters (with the receiver, for `C::m` of an
        // instance method).
        Symbol.MethodSymbol referenced(JCTree.JCMemberReference ref, int count) {
            if (ref.expr.type == null || !(ref.expr.type.tsym instanceof Symbol.ClassSymbol c)) return null;
            boolean type = ref.expr instanceof JCTree.JCIdent id && id.sym instanceof Symbol.ClassSymbol
                || ref.expr instanceof JCTree.JCFieldAccess fa && fa.sym instanceof Symbol.ClassSymbol
                || ref.expr instanceof JCTree.JCTypeApply;
            Symbol.MethodSymbol found = null;
            Iterable<Symbol> members = ref.name == ref.name.table.names.init ? c.members().getSymbolsByName(ref.name)
                : types.membersClosure(c.type, false).getSymbolsByName(ref.name);
            for (Symbol member : members) {
                if (!(member instanceof Symbol.MethodSymbol m) || m.isVarArgs()) continue;
                int n = m.params().size() + (type && !m.isStatic() && !m.isConstructor() ? 1 : 0);
                if (n != count) continue;
                if (found != null && !found.overrides(m, c, types, false) && !m.overrides(found, c, types, false)) return null;
                if (found == null || m.overrides(found, c, types, false)) found = m;
            }
            return found;
        }

        // `t` with each of `variables` in it as a type argument made `?`; null where one is `t`
        // itself or is anywhere else.
        Type wildcarded(Type t, List<Type> variables) {
            if (!mentions(t, variables)) return t;
            if (!t.hasTag(TypeTag.CLASS) || t.isCompound()) return null;
            List<Type> args = new ArrayList<>();
            for (Type a : t.getTypeArguments()) {
                if (variables.contains(a)) {
                    args.add(new Type.WildcardType(syms.objectType, com.sun.tools.javac.code.BoundKind.UNBOUND, syms.boundClass));
                } else if (a.hasTag(TypeTag.WILDCARD)) {
                    if (mentions(a, variables)) return null;
                    args.add(a);
                } else {
                    Type inner = wildcarded(a, variables);
                    if (inner == null) return null;
                    args.add(inner);
                }
            }
            if (mentions(t.getEnclosingType(), variables)) return null;
            return new Type.ClassType(t.getEnclosingType(), list(args), t.tsym);
        }

        // The parameter types of a call of `m` with `count` arguments, the variable arity
        // parameter spread over the arguments it takes.
        static List<Type> parameters(Symbol.MethodSymbol m, Type type, int count) {
            List<Type> params = new ArrayList<>(type.getParameterTypes());
            if (m.isVarArgs() && !params.isEmpty() && params.size() != count) {
                Type last = params.remove(params.size() - 1);
                Type element = last.hasTag(TypeTag.ARRAY) ? ((Type.ArrayType) last).elemtype : last;
                while (params.size() < count) params.add(element);
            }
            return params.size() == count ? params : null;
        }

        // The class of the value `expr` has, as far as its declarations tell.
        Symbol.ClassSymbol classOf(JCTree.JCExpression expr) {
            if (TreeInfo.symbol(expr) instanceof Symbol.ClassSymbol named) return named;
            Type t = known(expr);
            if (t == null && expr instanceof JCTree.JCMethodInvocation call && resolve(call) instanceof Symbol.MethodSymbol m)
                t = m.getReturnType();
            if (t == null && expr instanceof JCTree.JCNewClass created && created.clazz.type != null) t = created.clazz.type;
            if (t == null && expr instanceof JCTree.JCParens parens) return classOf(parens.expr);
            return t != null && t.tsym instanceof Symbol.ClassSymbol c ? c : null;
        }

        // How many parameters a lambda or method reference can take, or null for any: a method
        // reference `C::m` takes m's parameters, and the receiver first for an instance method.
        Set<Integer> arity(JCTree.JCExpression function) {
            if (function instanceof JCTree.JCLambda lambda) return Set.of(lambda.params.size());
            if (!(function instanceof JCTree.JCMemberReference ref) || ref.expr.type == null
                    || !(ref.expr.type.tsym instanceof Symbol.ClassSymbol c)) return null;
            boolean type = ref.expr instanceof JCTree.JCIdent id && id.sym instanceof Symbol.ClassSymbol
                || ref.expr instanceof JCTree.JCFieldAccess fa && fa.sym instanceof Symbol.ClassSymbol
                || ref.expr instanceof JCTree.JCTypeApply;
            Set<Integer> counts = new HashSet<>();
            for (Symbol member : types.membersClosure(c.type, false).getSymbolsByName(ref.name)) {
                if (!(member instanceof Symbol.MethodSymbol m)) continue;
                if (m.isVarArgs()) return null;
                int n = m.params().size();
                counts.add(type && !m.isStatic() && !m.isConstructor() ? n + 1 : n);
            }
            return counts.isEmpty() ? null : counts;
        }

        // The method `call` calls: javac's, or where javac resolved none (its receiver's type
        // being erroneous), the one method of the receiver's class of that name that the
        // arguments fit by number, lambdas by their parameters.
        Symbol resolve(JCTree.JCMethodInvocation call) {
            Symbol s = TreeInfo.symbol(call.meth);
            if (s instanceof Symbol.MethodSymbol || !(call.meth instanceof JCTree.JCFieldAccess access)) return s;
            Symbol.ClassSymbol c = classOf(access.selected);
            if (c == null) return s;
            Symbol.MethodSymbol found = null;
            for (Symbol member : types.membersClosure(c.type, false).getSymbolsByName(access.name)) {
                if (!(member instanceof Symbol.MethodSymbol m) || m.isConstructor()) continue;
                List<Type> params = parameters(m, m.type, call.args.size());
                if (params == null) continue;
                boolean fits = true;
                for (int i = 0; i < params.size() && fits; i++) {
                    Set<Integer> n = arity(call.args.get(i));
                    if (n == null) continue;
                    Type descriptor = descriptor(params.get(i));
                    fits = descriptor != null && n.contains(descriptor.getParameterTypes().size());
                }
                if (!fits) continue;
                if (found != null && found != m && !found.overrides(m, c, types, false) && !m.overrides(found, c, types, false)) return s;
                if (found == null || m.overrides(found, c, types, false)) found = m;
            }
            return found != null ? found : s;
        }

        Type call(JCTree.JCMethodInvocation call, Type target) {
            Symbol s = resolve(call);
            JCTree.JCExpression receiver = call.meth instanceof JCTree.JCFieldAccess access ? access.selected : null;
            if (!(s instanceof Symbol.MethodSymbol m)) {
                if (receiver != null) down(receiver, null);
                for (JCTree.JCExpression arg : call.args) down(arg, null);
                return null;
            }
            // The receiver's type, worked out from below where it needs inference.
            Type site = null;
            boolean trusted = m.isStatic() || receiver == null || known(receiver) != null;
            if (!m.isStatic()) {
                site = siteOf(call, m);
                if (site == null && receiver != null && !(fromTarget && target != null && writable(target))) {
                    Map<Integer, Edit> kept = new TreeMap<>(edits);
                    Type below = down(receiver, null);
                    edits.clear();
                    edits.putAll(kept);
                    if (below != null && below.hasTag(TypeTag.CLASS) && writable(below)) site = types.asSuper(below, m.owner);
                    // Where the result on that receiver is not what the call is for, the original
                    // had type arguments in the receiver that the target then gives.
                    if (site != null && target != null && writable(target) && m.getTypeParameters().isEmpty()
                            && !types.isAssignable(types.memberType(site, m).getReturnType(), target))
                        site = null;
                }
            }
            Set<Type> unknown = new LinkedHashSet<>();
            Type type;
            if (site != null) type = types.memberType(upperArguments(site), m);
            else {
                type = m.type;
                if (!m.isStatic()) unknown.addAll(m.owner.type.getTypeArguments());
            }
            List<Type> variables = type.getTypeArguments();
            unknown.addAll(variables);
            Constraints k = new Constraints(unknown);
            if (target != null) k.unify(type.getReturnType(), target, Way.BELOW);
            Map<Type, Type> bound = k.resolve();
            List<Type> params = parameters(m, type, call.args.size());
            if (params == null) return null;
            for (int round = 0; round < 3; round++) {
                bound = k.resolve();
                for (int i = 0; i < call.args.size(); i++) {
                    JCTree.JCExpression arg = call.args.get(i);
                    Type param = params.get(i);
                    bound = k.resolve();
                    if (arg instanceof JCTree.JCLambda || arg instanceof JCTree.JCMemberReference) {
                        Type descriptor = descriptor(subst(param, bound));
                        Type pattern = descriptor(param);
                        if (descriptor == null || pattern == null || descriptor.getParameterTypes().stream().anyMatch(p -> mentions(p, unknown)))
                            continue;
                        Type result = resultOf(arg, descriptor);
                        if (result != null) k.unify(pattern.getReturnType(), result, Way.ABOVE);
                    } else {
                        Type t = known(arg);
                        if (t == null && round < 2) {
                            Map<Integer, Edit> kept = new TreeMap<>(edits);
                            t = down(arg, subst(param, bound));
                            edits.clear();
                            edits.putAll(kept);
                        }
                        if (t != null) k.unify(param, t, Way.ABOVE);
                    }
                }
                // A bound of a variable that is known gives the variables in it (`M extends Map<K, V>`).
                for (Type v : variables) {
                    Type value = bound.get(v);
                    if (value == null) continue;
                    for (Type b : types.getBounds((Type.TypeVar) v))
                        if (mentions(b, unknown)) k.unify(b, value, Way.ABOVE);
                }
            }
            bound = k.resolve();
            // The type arguments the decompiler wrote count where nothing else gives them.
            boolean written = !call.typeargs.isEmpty() && call.typeargs.size() == variables.size();
            boolean differ = false;
            if (written) {
                for (int i = 0; i < variables.size(); i++) {
                    Type given = call.typeargs.get(i).type;
                    Type worked = bound.get(variables.get(i));
                    if (worked == null) {
                        if (given != null && !given.isErroneous()) bound.put(variables.get(i), given);
                    } else if (given == null || !types.isSameType(given, worked)) {
                        differ = true;
                    }
                }
            }
            boolean all = !variables.isEmpty() && variables.stream().allMatch(bound::containsKey);
            if (all && (written ? differ : named == null || named.contains(m))) {
                witness(call, m, variables, bound, written);
                // An argument whose declared type the parameter's then does not take, cast to
                // it, as the class file's unchecked conversion had it (`(U) this.value`).
                for (int i = 0; i < call.args.size(); i++) {
                    JCTree.JCExpression arg = call.args.get(i);
                    Type t = known(arg), param = subst(params.get(i), bound);
                    if (t == null || t.hasTag(TypeTag.BOT) || !writable(param) || types.isAssignable(t, param)) continue;
                    try {
                        Edit edit = cast(types, cu, arg, call, t, param);
                        if (edit != null) edits.put(edit.start(), edit);
                    } catch (Unprintable e) {
                        // left as it is
                    }
                }
                // A lambda whose body's declared type the result of its function type then does
                // not take, its body cast to that result, unchecked (`e -> (Optic<? super P, ?,
                // ?, ?, ?>) (Optic<?, ?, ?, ?, ?>) e.optic()`).
                for (int i = 0; i < call.args.size(); i++) {
                    if (!(call.args.get(i) instanceof JCTree.JCLambda lambda) || !(lambda.body instanceof JCTree.JCExpression body))
                        continue;
                    Type descriptor = descriptor(subst(params.get(i), bound));
                    if (descriptor == null) continue;
                    Type result = descriptor.getReturnType();
                    Type t = resultOf(lambda, descriptor);
                    if (t == null || t.hasTag(TypeTag.BOT) || result.hasTag(TypeTag.VOID) || !writable(result)
                            || types.isAssignable(t, result)) continue;
                    try {
                        Edit edit = cast(types, cu, body, lambda, t, result);
                        if (edit != null) edits.put(edit.start(), edit);
                    } catch (Unprintable e) {
                        // left as it is
                    }
                }
            }
            // The receiver, for the type its class variables were found to have.
            if (receiver != null && !trusted) {
                List<Type> classVariables = m.owner.type.getTypeArguments();
                Type wanted = site == null && !classVariables.isEmpty() ? subst(m.owner.type, bound) : null;
                down(receiver, wanted);
            }
            for (int i = 0; i < call.args.size(); i++) down(call.args.get(i), subst(params.get(i), bound));
            Type result = subst(type.getReturnType(), bound);
            return result == null || mentions(result, unknown) || result.isErroneous() ? null : result;
        }

        // `site` with each wildcard type argument made a type it holds (`Stream<Element<?, ?>>`
        // for `Stream<? extends Element<?, ?>>`): its bound, or for `?`, the erasure of the
        // class's variable's bound. The members of a wildcard type are those of such a type.
        Type upperArguments(Type site) {
            if (!site.hasTag(TypeTag.CLASS) || site.getTypeArguments().stream().noneMatch(a -> a.hasTag(TypeTag.WILDCARD)))
                return site;
            List<Type> args = new ArrayList<>();
            var formals = site.tsym.type.getTypeArguments();
            for (int i = 0; i < site.getTypeArguments().size(); i++) {
                Type a = site.getTypeArguments().get(i);
                if (a.hasTag(TypeTag.WILDCARD)) {
                    Type.WildcardType w = (Type.WildcardType) a;
                    a = w.kind == com.sun.tools.javac.code.BoundKind.UNBOUND
                        ? types.erasure(formals.get(i).getUpperBound()) : w.type;
                }
                args.add(a);
            }
            return new Type.ClassType(site.getEnclosingType(), list(args), site.tsym);
        }

        // `Q.<T>m(..)` for `Q.m(..)` (or for the type arguments written), `Owner.<T>m(..)` or
        // `this.<T>m(..)` for `m(..)`.
        void witness(JCTree.JCMethodInvocation call, Symbol.MethodSymbol m, List<Type> variables, Map<Type, Type> bound,
                boolean written) {
            List<String> printed = new ArrayList<>();
            try {
                for (Type v : variables) printed.add(print(cu, bound.get(v), false));
            } catch (Unprintable e) {
                return;
            }
            String arguments = "<" + String.join(", ", printed) + ">";
            int start, end;
            String text = arguments;
            if (call.meth instanceof JCTree.JCFieldAccess access) {
                int after = TreeInfo.getEndPos(access.selected, cu.endPositions);
                String between = source(cu, after, TreeInfo.getEndPos(access, cu.endPositions));
                int dot = between.indexOf('.');
                if (dot < 0) return;
                start = after + dot + 1;
                end = start;
                if (written) {
                    int open = between.indexOf('<', dot), close = between.lastIndexOf('>');
                    if (open < 0 || close < open) return;
                    start = after + open;
                    end = after + close + 1;
                }
            } else if (call.meth instanceof JCTree.JCIdent ident && !written) {
                start = end = TreeInfo.getStartPos(ident);
                JCTree.JCClassDecl c = enclosingClass(cu, call);
                try {
                    if (m.isStatic()) text = printClass(cu, (Symbol.ClassSymbol) m.owner) + "." + arguments;
                    else if (c != null && c.sym != null && c.sym.isSubClass(m.owner, types)) text = "this." + arguments;
                    else return;
                } catch (Unprintable e) {
                    return;
                }
            } else {
                return;
            }
            edits.put(start, new Edit(start, end, text, "type arguments " + arguments + " for " + m.name));
        }

        // `new C<T>(..)` for a raw `new C(..)` or for type arguments that disagree with `target`.
        Type created(JCTree.JCNewClass created, Type target) {
            Symbol.ClassSymbol c = created.clazz.type == null || created.clazz.type.isErroneous() ? null
                : (Symbol.ClassSymbol) created.clazz.type.tsym;
            // a diamond that javac could not infer leaves the class only in the name
            if (c == null && created.clazz instanceof JCTree.JCTypeApply apply
                    && TreeInfo.symbol(apply.clazz) instanceof Symbol.ClassSymbol written) c = written;
            if (c == null) {
                for (JCTree.JCExpression arg : created.args) down(arg, null);
                return null;
            }
            // javac's constructor, or where it resolved none, the one that takes as many arguments.
            Symbol.MethodSymbol constructor = created.constructor instanceof Symbol.MethodSymbol m ? m : null;
            if (constructor == null) {
                for (Symbol member : c.members().getSymbolsByName(c.name.table.names.init)) {
                    if (member instanceof Symbol.MethodSymbol m && parameters(m, m.type, created.args.size()) != null) {
                        if (constructor != null) {
                            constructor = null;
                            break;
                        }
                        constructor = m;
                    }
                }
            }
            List<Type> variables = c.type.getTypeArguments();
            Constraints k = new Constraints(new LinkedHashSet<>(variables));
            if (target != null && !variables.isEmpty()) k.unify(c.type, target, Way.BELOW);
            Map<Type, Type> bound = k.resolve();
            JCTree.JCExpression clazz = created.clazz instanceof JCTree.JCAnnotatedType annotated ? annotated.underlyingType : created.clazz;
            boolean all = !variables.isEmpty() && variables.stream().allMatch(bound::containsKey), castArguments = false;
            if (all && created.def == null && created.encl == null) {
                Type wanted = subst(c.type, bound);
                if (clazz instanceof JCTree.JCTypeApply apply) {
                    Type written = apply.type != null ? apply.type : created.clazz.type;
                    // Type arguments that disagree, or for every call, a diamond that javac
                    // cannot infer, written out; the arguments the constructor then does not
                    // take are cast to its parameters.
                    boolean diamond = apply.arguments.isEmpty() && named == null;
                    if (diamond || !apply.arguments.isEmpty() && written != null && !types.isSameType(written, wanted)) {
                        try {
                            List<String> printed = new ArrayList<>();
                            for (Type v : variables) printed.add(print(cu, bound.get(v), false));
                            int start = TreeInfo.getEndPos(apply.clazz, cu.endPositions), end = TreeInfo.getEndPos(apply, cu.endPositions);
                            String arguments = "<" + String.join(", ", printed) + ">";
                            edits.put(start, new Edit(start, end, arguments, "type arguments " + arguments + " for new " + c.name));
                            castArguments = diamond;
                        } catch (Unprintable e) {
                            // left as written
                        }
                    }
                } else {
                    try {
                        List<String> printed = new ArrayList<>();
                        for (Type v : variables) printed.add(print(cu, bound.get(v), false));
                        int end = TreeInfo.getEndPos(clazz, cu.endPositions);
                        String arguments = "<" + String.join(", ", printed) + ">";
                        edits.put(end, new Edit(end, end, arguments, "type arguments " + arguments + " for new " + c.name));
                    } catch (Unprintable e) {
                        // left raw
                    }
                }
            } else if (clazz instanceof JCTree.JCTypeApply apply && !apply.arguments.isEmpty() && apply.type != null) {
                for (int i = 0; i < variables.size() && i < apply.type.getTypeArguments().size(); i++)
                    bound.putIfAbsent(variables.get(i), apply.type.getTypeArguments().get(i));
            }
            List<Type> params = constructor == null ? null : parameters(constructor, constructor.type, created.args.size());
            if (castArguments && params != null) {
                for (int i = 0; i < created.args.size(); i++) {
                    JCTree.JCExpression arg = created.args.get(i);
                    Type t = standalone(arg), param = subst(params.get(i), bound);
                    if (t == null || t.hasTag(TypeTag.BOT) || !writable(param) || types.isAssignable(t, param)) continue;
                    try {
                        Edit edit = cast(types, cu, arg, created, t, param);
                        if (edit != null) edits.put(edit.start(), edit);
                    } catch (Unprintable e) {
                        // left as it is
                    }
                }
            }
            for (int i = 0; i < created.args.size(); i++)
                down(created.args.get(i), params == null ? null : subst(params.get(i), bound));
            return all ? subst(c.type, bound) : known(created);
        }
    }

    // The path to `tree` in the unit, or where javac reported the error at a copy of the unit's
    // tree that it attributed on the side (an argument of an inference that failed), to the tree
    // of the unit of the same kind at the same place.
    static TreePath pathTo(CompilationUnitTree unit, JCTree tree) {
        TreePath path = TreePath.getPath(unit, tree);
        if (path != null) return path;
        JCTree.JCCompilationUnit cu = (JCTree.JCCompilationUnit) unit;
        int start = TreeInfo.getStartPos(tree);
        String text = tree.toString();
        TreePath[] found = {null};
        new TreePathScanner<Void, Void>() {
            public Void scan(Tree t, Void v) {
                if (found[0] != null) return null;
                if (t instanceof JCTree j) {
                    if (TreeInfo.getStartPos(j) > start || TreeInfo.getEndPos(j, cu.endPositions) < start) return null;
                    if (j.getKind() == tree.getKind() && TreeInfo.getStartPos(j) == start && j.toString().equals(text)) {
                        found[0] = new TreePath(getCurrentPath(), t);
                        return null;
                    }
                }
                return super.scan(t, v);
            }
        }.scan(unit, null);
        return found[0];
    }

    // The outermost expression around `path` below its statement.
    static TreePath expressionOf(TreePath path) {
        TreePath at = path;
        while (at.getParentPath() != null && !(at.getParentPath().getLeaf() instanceof StatementTree)
                && !(at.getParentPath().getLeaf() instanceof JCTree.JCLambda)
                && !(at.getParentPath().getLeaf() instanceof ClassTree))
            at = at.getParentPath();
        return at.getLeaf() instanceof JCTree.JCExpression ? at : null;
    }

    // For each cast of the decompiler's to a raw class or an array of one (`(Comparable) value`,
    // `(Enum[]) keys`) in the expression of an error: the cast to each type variable in scope
    // whose erasure that is, one option each. A cast to a type variable has the checkcast of its
    // erasure, the same as the raw cast.
    static List<List<Edit>> typeVariableCasts(Types types, JCTree.JCCompilationUnit cu, TreePath path) {
        TreePath top = expressionOf(path);
        if (top == null) return List.of();
        Witnesses w = new Witnesses(types, null, cu, null);
        w.scopeOf(top);
        List<List<Edit>> options = new ArrayList<>();
        new TreeScanner<Void, Void>() {
            public Void visitTypeCast(TypeCastTree node, Void v) {
                JCTree.JCTypeCast cast = (JCTree.JCTypeCast) node;
                Type to = cast.clazz.type;
                int dims = 0;
                while (to != null && to.hasTag(TypeTag.ARRAY)) {
                    to = ((Type.ArrayType) to).elemtype;
                    dims++;
                }
                if (to != null && to.hasTag(TypeTag.CLASS) && to.isRaw()) {
                    for (Symbol variable : w.scope) {
                        if (!types.isSameType(types.erasure(variable.type), to)) continue;
                        int start = TreeInfo.getStartPos(cast.clazz), end = TreeInfo.getEndPos(cast.clazz, cu.endPositions);
                        String text = variable.name + "[]".repeat(dims);
                        options.add(List.of(new Edit(start, end, text, "cast to the type variable " + text + " for "
                            + source(cu, start, end))));
                    }
                }
                return super.visitTypeCast(node, v);
            }
        }.scan(top.getLeaf(), null);
        return options;
    }

    static boolean capturing(Type t) {
        if (t == null) return false;
        if (t instanceof Type.CapturedType) return true;
        if (t.hasTag(TypeTag.CLASS)) {
            for (Type a : t.getTypeArguments()) if (capturing(a)) return true;
        }
        if (t.hasTag(TypeTag.WILDCARD)) return capturing(((Type.WildcardType) t).type);
        if (t.hasTag(TypeTag.ARRAY)) return capturing(((Type.ArrayType) t).elemtype);
        return false;
    }

    // The call at or around `path`.
    static JCTree.JCMethodInvocation callAround(TreePath path) {
        for (TreePath at = path; at != null; at = at.getParentPath()) {
            if (at.getLeaf() instanceof JCTree.JCMethodInvocation call) return call;
            if (at.getLeaf() instanceof StatementTree || at.getLeaf() instanceof JCTree.JCLambda) return null;
        }
        return null;
    }

    // The arguments of the call at the error whose types hold a captured wildcard
    // (`List<Pair<String, Dynamic<CAP#1>>>` for a parameter `List<Pair<String, Dynamic<?>>>`),
    // cast to their raw class: that converts unchecked, with no checkcast.
    static List<Edit> rawCapturedArguments(JCTree.JCCompilationUnit cu, TreePath path) throws Unprintable {
        JCTree.JCMethodInvocation call = callAround(path);
        if (call == null) return List.of();
        List<Edit> edits = new ArrayList<>();
        for (JCTree.JCExpression arg : call.args)
            if (arg.type != null && arg.type.hasTag(TypeTag.CLASS) && arg.type.isParameterized() && capturing(arg.type))
                edits.add(wrap(cu, arg, printClass(cu, (Symbol.ClassSymbol) arg.type.tsym), "cast to raw "));
        return edits;
    }

    // The lambdas and method references passed to the call at the error, cast to the interface
    // that the class file makes them for, where one such interface fits the parameter of a method
    // of the call's name and arity (`(Fixer) Fix::fixBucket` for a `Function<Typed<?>, Typed<?>>`).
    static List<Edit> functionalArguments(Types types, JCTree.JCCompilationUnit cu, TreePath path,
            java.util.function.Function<String, Symbol.ClassSymbol> named) throws Unprintable {
        JCTree.JCMethodInvocation call = callAround(path);
        if (call == null) return List.of();
        Type site = call.meth instanceof JCTree.JCFieldAccess access ? access.selected.type : null;
        if (site == null) {
            JCTree.JCClassDecl c = enclosingClass(cu, call);
            if (c == null || c.sym == null) return List.of();
            site = c.sym.type;
        }
        if (!site.hasTag(TypeTag.CLASS)) return List.of();
        Name name = TreeInfo.name(call.meth);
        List<Edit> edits = new ArrayList<>();
        for (int i = 0; i < call.args.size(); i++) {
            JCTree.JCExpression arg = call.args.get(i);
            if (!(arg instanceof JCTree.JCLambda || arg instanceof JCTree.JCMemberReference)) continue;
            Set<Edit> found = new LinkedHashSet<>();
            for (Symbol member : types.membersClosure(site, false).getSymbolsByName(name)) {
                if (!(member instanceof Symbol.MethodSymbol m) || m.params().size() != call.args.size()) continue;
                Edit edit = functionalCast(types, cu, arg, m.params().get(i).type, named);
                if (edit != null) found.add(edit);
            }
            if (found.size() == 1) edits.addAll(found);
        }
        return edits;
    }

    // `((Registry) registries).getId(registry)` for `registries.getId(registry)`, where the
    // argument's expected type is a capture of the receiver's wildcard: the call through the raw
    // receiver has the same descriptor.
    static Edit rawReceiver(JCTree.JCCompilationUnit cu, TreePath path) throws Unprintable {
        Tree parent = path.getParentPath() == null ? null : path.getParentPath().getLeaf();
        if (!(parent instanceof JCTree.JCMethodInvocation call) || !call.args.contains(path.getLeaf())
                || !(call.meth instanceof JCTree.JCFieldAccess access) || access.selected.type == null
                || !access.selected.type.hasTag(TypeTag.CLASS) || !access.selected.type.isParameterized())
            return null;
        JCTree.JCExpression receiver = access.selected;
        int start = TreeInfo.getStartPos(receiver), end = TreeInfo.getEndPos(receiver, cu.endPositions);
        String raw = printClass(cu, (Symbol.ClassSymbol) receiver.type.tsym);
        return new Edit(start, end, "((" + raw + ") " + source(cu, start, end) + ")",
            "receiver cast to raw " + raw + ": " + oneLine(source(cu, start, end)));
    }

    // A generic call that is the whole initializer or result of its declared type, which no
    // instance of its type variables gives (`Codec<Named<?, ?>> CODEC = typedCodec()` of a
    // `<P, R> Codec<Named<P, R>>`): cast to that type through the wildcard type, unchecked.
    static Edit declaredCast(Types types, JCTree.JCCompilationUnit cu, TreePath path) throws Unprintable {
        TreePath top = expressionOf(path);
        if (top == null || !(top.getLeaf() instanceof JCTree.JCMethodInvocation call)
                || !(TreeInfo.symbol(call.meth) instanceof Symbol.MethodSymbol m) || m.getTypeParameters().isEmpty())
            return null;
        Type declared = declaredFor(top);
        if (declared == null || !declared.hasTag(TypeTag.CLASS) || !declared.isParameterized()) return null;
        return cast(types, cu, call, top.getParentPath().getLeaf(), types.erasure(m.getReturnType()), declared);
    }

    // The methods whose type variables an inference error names as not inferred.
    static Set<Symbol> uninferred(JCDiagnostic diag) {
        Set<Symbol> found = new HashSet<>();
        collectUninferred(diag.getArgs(), found);
        return found;
    }

    static void collectUninferred(Object[] args, Set<Symbol> found) {
        for (Object arg : args) {
            if (arg instanceof JCDiagnostic detail) collectUninferred(detail.getArgs(), found);
            else if (arg instanceof Iterable<?> list) {
                for (Object item : list) collectUninferred(new Object[] {item}, found);
            } else if (arg instanceof Type.TypeVar v && !(v instanceof Type.CapturedType)
                    && v.tsym.owner instanceof Symbol.MethodSymbol m) {
                found.add(m);
            }
        }
    }

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
            // A cast to the expected type that javac rejects (the two parameterizations are
            // provably distinct): through the found class's wildcard type, an upcast with no
            // `checkcast`, so that the written cast keeps its own.
            if (parent instanceof JCTree.JCTypeCast written && written.expr == expr && written.clazz.type != null
                    && types.isSameType(written.clazz.type, expected) && found.hasTag(TypeTag.CLASS)) {
                Symbol.ClassSymbol c = (Symbol.ClassSymbol) found.tsym;
                String via = "(" + printClass(cu, c) + (c.type.getTypeArguments().isEmpty() ? ""
                    : "<" + c.type.getTypeArguments().stream().map(t -> "?").collect(Collectors.joining(", ")) + ">") + ") ";
                int start = TreeInfo.getStartPos(expr);
                return new Edit(start, start, via, "cast through " + via.trim() + ": "
                    + oneLine(source(cu, start, TreeInfo.getEndPos(expr, cu.endPositions))));
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
    // each invokedynamic of LambdaMetafactory), as binary names, also by the name of the method
    // each refers to; where each string constant is loaded; and its methods, by name and
    // descriptor, that are static.
    record ClassCode(Set<String> checkcasts, Set<String> lambdas, Map<String, Set<String>> referenced,
            Map<String, List<Constant>> strings, Set<String> statics) {}

    // A load of a string constant: in which method (by its index in the class file), the local
    // variable that the start of the nearest exception handler before it stores the exception in,
    // and the reference variable loaded right after it (-1 for none).
    record Constant(int method, int handler, int loaded) {}

    // The class file of class `owner` (a binary name), read once.
    static ClassCode classCode(String owner) {
        if (classes == null) return new ClassCode(Set.of(), Set.of(), Map.of(), Map.of(), Set.of());
        return classCodes.computeIfAbsent(owner, o -> {
            Set<String> checks = new HashSet<>(), lambdas = new HashSet<>(), statics = new HashSet<>();
            Map<String, Set<String>> referenced = new HashMap<>();
            Map<String, List<Constant>> strings = new HashMap<>();
            var entry = classes.getEntry(o.replace('.', '/') + ".class");
            if (entry != null) {
                byte[] bytes;
                try (InputStream in = classes.getInputStream(entry)) {
                    bytes = in.readAllBytes();
                } catch (IOException e) {
                    throw new UncheckedIOException(e);
                }
                int index = 0;
                for (var method : ClassFile.of().parse(bytes).methods()) {
                    index++;
                    if ((method.flags().flagsMask() & ClassFile.ACC_STATIC) != 0)
                        statics.add(method.methodName().stringValue() + method.methodType().stringValue());
                    CodeModel code = method.code().orElse(null);
                    if (code == null) continue;
                    Set<java.lang.classfile.Label> handlers = new HashSet<>();
                    for (var handler : code.exceptionHandlers()) handlers.add(handler.handler());
                    boolean atHandler = false;
                    int handlerSlot = -1, pendingHandler = -1;
                    String pending = null;
                    for (var element : code.elementList()) {
                        if (element instanceof LabelTarget target) {
                            if (handlers.contains(target.label())) atHandler = true;
                            continue;
                        }
                        if (!(element instanceof Instruction instruction)) continue;
                        if (pending != null) {
                            int loaded = instruction instanceof LoadInstruction load && load.typeKind() == TypeKind.REFERENCE
                                ? load.slot() : -1;
                            strings.computeIfAbsent(pending, s -> new ArrayList<>()).add(new Constant(index, pendingHandler, loaded));
                            pending = null;
                        }
                        if (atHandler) {
                            handlerSlot = instruction instanceof StoreInstruction store ? store.slot() : -1;
                            atHandler = false;
                        }
                        if (instruction instanceof ConstantInstruction.LoadConstantInstruction ldc
                                && ldc.constantValue() instanceof String s) {
                            pending = s;
                            pendingHandler = handlerSlot;
                        }
                        if (element instanceof TypeCheckInstruction check && check.opcode() == Opcode.CHECKCAST)
                            checks.add(check.type().asInternalName().replace('/', '.'));
                        if (element instanceof InvokeDynamicInstruction indy
                                && indy.bootstrapMethod().owner().descriptorString()
                                    .equals("Ljava/lang/invoke/LambdaMetafactory;")) {
                            String made = indy.typeSymbol().returnType().descriptorString();
                            made = made.substring(1, made.length() - 1).replace('/', '.');
                            lambdas.add(made);
                            if (indy.bootstrapArgs().size() > 1
                                    && indy.bootstrapArgs().get(1) instanceof java.lang.constant.DirectMethodHandleDesc handle)
                                referenced.computeIfAbsent(handle.methodName(), n -> new HashSet<>()).add(made);
                        }
                    }
                }
            }
            return new ClassCode(checks, lambdas, referenced, strings, statics);
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
        ClassCode code = classCode(owner.sym.flatname.toString());
        // A method reference's interface is one the class file makes for that method.
        Set<String> made = expr instanceof JCTree.JCMemberReference ref
            ? code.referenced().getOrDefault(ref.name.toString(), Set.of()) : code.lambdas();
        for (String interfaceName : made) {
            Symbol.ClassSymbol sub = named.apply(interfaceName);
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

    // A method reference around the error whose qualifier has the raw type of a class C with one
    // type variable, which the referenced method returns, where the function type wants `R`
    // (DETAIL, javac's "found cannot be converted to R"): the type argument R for the one raw
    // `new C(..)` in the qualifier (`Util.make(new Int2ObjectOpenHashMap<String>(), ..)::get`).
    static List<Edit> typedRawQualifier(Types types, JCTree.JCCompilationUnit cu, TreePath path, JCDiagnostic detail)
            throws Unprintable {
        if (detail == null || !(detail.getArgs()[1] instanceof Type required) || !required.hasTag(TypeTag.CLASS))
            return List.of();
        while (path != null && !(path.getLeaf() instanceof JCTree.JCMemberReference)) {
            if (path.getLeaf() instanceof JCTree.JCStatement) return List.of();
            path = path.getParentPath();
        }
        if (path == null) return List.of();
        JCTree.JCMemberReference reference = (JCTree.JCMemberReference) path.getLeaf();
        Type qualifier = reference.expr.type;
        if (!(reference.sym instanceof Symbol.MethodSymbol method) || qualifier == null
                || !qualifier.hasTag(TypeTag.CLASS) || !qualifier.isRaw()) return List.of();
        List<Type> variables = qualifier.tsym.type.getTypeArguments();
        Type owner = types.asSuper(qualifier.tsym.type, method.owner);
        int returned = method.owner.type.getTypeArguments().stream().map(t -> t.tsym).toList()
            .indexOf(method.getReturnType().tsym);
        if (variables.size() != 1 || owner == null || returned < 0
                || owner.getTypeArguments().get(returned).tsym != variables.get(0).tsym) return List.of();
        List<JCTree.JCNewClass> made = new ArrayList<>();
        new TreeScanner<Void, Void>() {
            public Void visitNewClass(NewClassTree node, Void v) {
                JCTree.JCNewClass created = (JCTree.JCNewClass) node;
                if (!(created.clazz instanceof JCTree.JCTypeApply) && created.clazz.type != null
                        && created.clazz.type.tsym == qualifier.tsym) made.add(created);
                return super.visitNewClass(node, v);
            }
        }.scan(reference.expr, null);
        if (made.size() != 1) return List.of();
        String argument = print(cu, required, false);
        JCTree.JCNewClass created = made.get(0);
        int end = TreeInfo.getEndPos(created.clazz, cu.endPositions);
        return List.of(new Edit(end, end, "<" + argument + ">", "type argument of a method reference's raw "
            + "qualifier: " + oneLine(source(cu, TreeInfo.getStartPos(created), end)) + "<" + argument + ">"));
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
        // for all: with two (a use in a nested catch), either could be the variable it reads, and
        // the class file tells which.
        JCTree.JCCatch chosen = null;
        for (List<JCTree.JCCatch> catches : around) {
            List<JCTree.JCCatch> unused = catches.stream().filter(c -> !usesVariable(c)).toList();
            if (unused.size() != 1 || (chosen != null && unused.get(0) != chosen)) {
                chosen = catchInClassFile(cu, uses, around);
                break;
            }
            chosen = unused.get(0);
        }
        if (chosen == null) return List.of();
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

    // Of the catches around every use of an unresolved name, the one whose exception the class
    // file's code reads there. A string constant just before a use (`warn("Failed ..", ex)`) is
    // loaded right before the variable the use reads; a string constant of a catch's own block
    // comes after the start of its handler, which stores the exception in that catch's variable.
    // Each constant must be in the class file once, all in the same method.
    static JCTree.JCCatch catchInClassFile(JCTree.JCCompilationUnit cu, List<JCTree.JCIdent> uses,
            List<List<JCTree.JCCatch>> around) {
        JCTree.JCClassDecl owner = enclosingClass(cu, uses.get(0));
        if (owner == null || owner.sym == null) return null;
        ClassCode code = classCode(owner.sym.flatname.toString());
        Constant read = null;
        for (JCTree.JCIdent use : uses) {
            String before = stringBefore(cu, use);
            if (before == null) continue;
            List<Constant> found = code.strings().getOrDefault(before, List.of());
            if (found.size() != 1 || found.get(0).loaded() < 0) return null;
            Constant constant = found.get(0);
            if (read != null && (read.method() != constant.method() || read.loaded() != constant.loaded())) return null;
            read = constant;
        }
        if (read == null) return null;
        List<JCTree.JCCatch> common = new ArrayList<>(around.get(0));
        for (List<JCTree.JCCatch> catches : around) common.retainAll(catches);
        JCTree.JCCatch chosen = null;
        for (JCTree.JCCatch c : common) {
            String first = firstString(c.body);
            List<Constant> found = first == null ? List.of() : code.strings().getOrDefault(first, List.of());
            if (found.size() != 1 || found.get(0).method() != read.method()) continue;
            if (found.get(0).handler() == read.loaded()) {
                if (chosen != null) return null;
                chosen = c;
            }
        }
        return chosen;
    }

    // The string literal that is the argument just before `use` in a call or a creation.
    static String stringBefore(JCTree.JCCompilationUnit cu, JCTree.JCIdent use) {
        TreePath path = TreePath.getPath(cu, use);
        if (path == null || path.getParentPath() == null) return null;
        Tree parent = path.getParentPath().getLeaf();
        List<JCTree.JCExpression> args = parent instanceof JCTree.JCMethodInvocation call ? call.args
            : parent instanceof JCTree.JCNewClass created ? created.args : List.of();
        int i = args.indexOf(use);
        return i > 0 && args.get(i - 1) instanceof JCTree.JCLiteral literal && literal.getValue() instanceof String s ? s : null;
    }

    // The first string literal of a catch block, outside the catches, lambdas and classes in it,
    // whose code is elsewhere than right after the handler's start.
    static String firstString(JCTree.JCBlock body) {
        String[] found = {null};
        new TreeScanner<Void, Void>() {
            public Void scan(Tree t, Void v) {
                return found[0] != null ? null : super.scan(t, v);
            }
            public Void visitLiteral(LiteralTree node, Void v) {
                if (node.getValue() instanceof String s) found[0] = s;
                return null;
            }
            public Void visitCatch(CatchTree node, Void v) { return null; }
            public Void visitLambdaExpression(LambdaExpressionTree node, Void v) { return null; }
            public Void visitClass(ClassTree node, Void v) { return null; }
        }.scan(body, null);
        return found[0];
    }

    // An unresolved name that is assigned (`var24 = e.steps();`, where the decompiler left out the
    // declaration of a temporary), declared with the type of the value first assigned to it, at the
    // statement of the innermost block that holds every use of the name, if the first use is the
    // assignment and its value's type can be written.
    static Edit declareAssigned(Types types, JCTree.JCCompilationUnit cu, TreePath path, String name, Set<Integer> done)
            throws Unprintable {
        TreePath body = path;
        while (body != null && !(body.getLeaf() instanceof MethodTree) && !(body.getLeaf() instanceof LambdaExpressionTree))
            body = body.getParentPath();
        if (body == null) return null;
        List<TreePath> uses = new ArrayList<>();
        new TreePathScanner<Void, Void>() {
            public Void visitIdentifier(IdentifierTree node, Void v) {
                JCTree.JCIdent ident = (JCTree.JCIdent) node;
                if (ident.name.contentEquals(name) && unresolved(ident)) uses.add(getCurrentPath());
                return null;
            }
            public Void visitClass(ClassTree node, Void v) { return null; }
        }.scan(body, null);
        if (uses.isEmpty()) return null;
        TreePath first = uses.get(0);
        if (!(first.getParentPath().getLeaf() instanceof JCTree.JCAssign assign) || assign.lhs != first.getLeaf()
                || !(first.getParentPath().getParentPath().getLeaf() instanceof JCTree.JCExpressionStatement)) return null;
        // javac gives the value the erroneous type of the name it is assigned to, so its type is
        // the one that its declarations give.
        Type type = new Witnesses(types, null, cu, null).known(assign.rhs);
        if (type == null) type = assign.rhs.type;
        if (type == null || type.isErroneous() || type.hasTag(TypeTag.BOT) || type.hasTag(TypeTag.VOID)) return null;
        String written = print(cu, type, false);
        // The innermost block around every use, and its statement that holds the first.
        JCTree.JCBlock block = null;
        JCTree.JCStatement statement = null;
        for (TreePath up = first.getParentPath(); up.getParentPath() != null; up = up.getParentPath()) {
            if (up.getParentPath().getLeaf() instanceof JCTree.JCBlock b && up.getLeaf() instanceof JCTree.JCStatement s
                    && uses.stream().allMatch(u -> within(cu, u.getLeaf(), b))) {
                block = b;
                statement = s;
                break;
            }
            if (up.getLeaf() == body.getLeaf()) return null;
        }
        if (block == null) return null;
        for (TreePath use : uses) done.add(((JCTree) use.getLeaf()).getStartPosition());
        int start = TreeInfo.getStartPos(statement);
        return new Edit(start, start, written + " " + name + "; ", "declared " + written + " " + name + " that is assigned");
    }

    static boolean within(JCTree.JCCompilationUnit cu, Tree inner, JCTree outer) {
        return TreeInfo.getStartPos(outer) <= TreeInfo.getStartPos((JCTree) inner)
            && TreeInfo.getEndPos((JCTree) inner, cu.endPositions) <= TreeInfo.getEndPos(outer, cu.endPositions);
    }

    // A local variable declared again in the scope of one of the same name (the decompiler gave
    // two variables one name), renamed with the uses of the name after it in its block to a name
    // the file does not use.
    static List<Edit> renameRedeclared(JCTree.JCCompilationUnit cu, TreePath path, JCTree.JCVariableDecl variable) {
        if (!(path.getParentPath().getLeaf() instanceof JCTree.JCBlock block)) return List.of();
        String name = variable.name.toString(), text;
        try {
            text = cu.getSourceFile().getCharContent(true).toString();
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
        int n = 2;
        while (java.util.regex.Pattern.compile("\\b" + name + n + "\\b").matcher(text).find()) n++;
        String fresh = name + n;
        int at = variable.pos;
        if (!text.startsWith(name, at)) return List.of();
        List<Edit> edits = new ArrayList<>();
        edits.add(new Edit(at, at + name.length(), fresh, "renamed the second " + name + " to " + fresh));
        boolean[] redeclared = {false};
        int from = TreeInfo.getEndPos(variable, cu.endPositions);
        new TreeScanner<Void, Void>() {
            public Void visitIdentifier(IdentifierTree node, Void v) {
                JCTree.JCIdent ident = (JCTree.JCIdent) node;
                if (ident.name.contentEquals(name) && ident.getStartPosition() >= at && !(ident.sym instanceof Symbol.MethodSymbol))
                    edits.add(new Edit(ident.getStartPosition(), ident.getStartPosition() + name.length(), fresh,
                        "renamed " + name + " to " + fresh));
                return null;
            }
            public Void visitVariable(VariableTree node, Void v) {
                if (node != variable && node.getName().contentEquals(name) && ((JCTree) node).getStartPosition() >= from)
                    redeclared[0] = true;
                return super.visitVariable(node, v);
            }
            public Void visitMethodInvocation(MethodInvocationTree node, Void v) {
                // a method of the name is not the variable
                JCTree.JCMethodInvocation call = (JCTree.JCMethodInvocation) node;
                if (!(call.meth instanceof JCTree.JCIdent)) scan(call.meth, v);
                scan(call.typeargs, v);
                scan(call.args, v);
                return null;
            }
        }.scan(block, null);
        return redeclared[0] ? List.of() : edits;
    }

    // `SwitchBootstraps.typeSwitch("typeSwitch", new Object[]{A.class, B.class}, x, n)`, CFR's
    // writing of the invokedynamic of a pattern switch, as what the bootstrap's call site
    // returns: -1 for null, else the index of the first class from `n` on that `x` is an instance
    // of, else the number of classes (`x == null ? -1 : n <= 0 && x instanceof A ? 0 : ..`). Only
    // for a selector and a restart index that are variables or constants, which can be read again.
    static Edit typeSwitch(JCTree.JCCompilationUnit cu, TreePath path) {
        JCTree.JCMethodInvocation call = null;
        for (TreePath up = path; up != null && !(up.getLeaf() instanceof StatementTree && !(up.getLeaf() instanceof JCTree.JCSwitch));
                up = up.getParentPath()) {
            if (up.getLeaf() instanceof JCTree.JCMethodInvocation c && c.meth instanceof JCTree.JCFieldAccess access
                    && access.name.contentEquals("typeSwitch")
                    && TreeInfo.symbol(access.selected) instanceof Symbol.ClassSymbol owner
                    && owner.getQualifiedName().contentEquals("java.lang.runtime.SwitchBootstraps")) {
                call = c;
                break;
            }
            if (up.getLeaf() instanceof JCTree.JCSwitch s) {
                JCTree.JCExpression selector = TreeInfo.skipParens(s.selector);
                if (selector instanceof JCTree.JCMethodInvocation c && c.meth instanceof JCTree.JCFieldAccess access
                        && access.name.contentEquals("typeSwitch")
                        && TreeInfo.symbol(access.selected) instanceof Symbol.ClassSymbol owner
                        && owner.getQualifiedName().contentEquals("java.lang.runtime.SwitchBootstraps"))
                    call = c;
                break;
            }
        }
        if (call == null || call.args.size() != 4 || !(call.args.get(1) instanceof JCTree.JCNewArray labels)
                || labels.elems == null) return null;
        JCTree.JCExpression selector = call.args.get(2), restart = call.args.get(3);
        JCTree.JCExpression read = selector;
        while (read instanceof JCTree.JCTypeCast || read instanceof JCTree.JCParens)
            read = read instanceof JCTree.JCTypeCast cast ? cast.expr : ((JCTree.JCParens) read).expr;
        if (!(read instanceof JCTree.JCIdent) || !(restart instanceof JCTree.JCIdent || restart instanceof JCTree.JCLiteral))
            return null;
        String x = "(" + source(cu, TreeInfo.getStartPos(selector), TreeInfo.getEndPos(selector, cu.endPositions)) + ")";
        String n = source(cu, TreeInfo.getStartPos(restart), TreeInfo.getEndPos(restart, cu.endPositions));
        StringBuilder out = new StringBuilder("(" + x + " == null ? -1 : ");
        int i = 0;
        for (JCTree.JCExpression label : labels.elems) {
            if (!(label instanceof JCTree.JCFieldAccess literal) || !literal.name.contentEquals("class")) return null;
            String type = source(cu, TreeInfo.getStartPos(literal.selected), TreeInfo.getEndPos(literal.selected, cu.endPositions));
            out.append(n).append(" <= ").append(i).append(" && ").append(x).append(" instanceof ").append(type)
                .append(" ? ").append(i).append(" : ");
            i++;
        }
        out.append(i).append(")");
        int start = TreeInfo.getStartPos(call), end = TreeInfo.getEndPos(call, cu.endPositions);
        return new Edit(start, end, out.toString(), "pattern switch index for " + oneLine(source(cu, start, end)));
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

    // Each method that the source does not declare static but whose class file has it static
    // (Procyon writes an interface's static method as a `default` one, which other files then
    // cannot call on the interface): `static` in place of `default`, or added. Edits the files;
    // returns whether it edited any.
    static boolean staticMethods(JavacTask task, List<CompilationUnitTree> units) throws IOException {
        if (classes == null) return false;
        Types types = Types.instance(((JavacTaskImpl) task).getContext());
        boolean any = false;
        for (CompilationUnitTree unit : units) {
            JCTree.JCCompilationUnit cu = (JCTree.JCCompilationUnit) unit;
            List<Edit> edits = new ArrayList<>();
            new TreeScanner<Void, Void>() {
                public Void visitMethod(MethodTree node, Void v) {
                    JCTree.JCMethodDecl method = (JCTree.JCMethodDecl) node;
                    if (method.sym != null && !method.sym.isStatic() && !method.sym.isConstructor()
                            && method.sym.owner instanceof Symbol.ClassSymbol owner && !method.type.isErroneous()) {
                        String descriptor = descriptor(types, method.sym.type);
                        if (descriptor != null && classCode(owner.flatname.toString()).statics()
                                .contains(method.name + descriptor)) {
                            int start = TreeInfo.getStartPos(method), name = method.pos;
                            String head = source(cu, start, name);
                            int at = (method.mods.flags & com.sun.tools.javac.code.Flags.DEFAULT) != 0
                                ? java.util.regex.Pattern.compile("\\bdefault\\b").matcher(head).results()
                                    .mapToInt(java.util.regex.MatchResult::start).findFirst().orElse(-1) : -1;
                            edits.add(at >= 0 ? new Edit(start + at, start + at + "default".length(), "static",
                                    "static as in the class file, for default: " + method.name)
                                : new Edit(start, start, "static ", "static as in the class file: " + method.name));
                        }
                    }
                    return super.visitMethod(node, v);
                }
            }.scan(cu, null);
            if (edits.isEmpty()) continue;
            Path file = Path.of(unit.getSourceFile().toUri());
            String text = Files.readString(file);
            for (Edit edit : edits)
                log.printf("round 0 %s:%d %s%n", root.relativize(file), line(text, edit.start()), edit.what());
            Files.writeString(file, apply(text, edits));
            any = true;
        }
        return any;
    }

    // The JVM descriptor of a type's erasure, or null where it has none.
    static String descriptor(Types types, Type t) {
        t = types.erasure(t);
        return switch (t.getTag()) {
            case BYTE -> "B";
            case CHAR -> "C";
            case SHORT -> "S";
            case INT -> "I";
            case LONG -> "J";
            case FLOAT -> "F";
            case DOUBLE -> "D";
            case BOOLEAN -> "Z";
            case VOID -> "V";
            case ARRAY -> {
                String element = descriptor(types, ((Type.ArrayType) t).elemtype);
                yield element == null ? null : "[" + element;
            }
            case CLASS -> "L" + ((Symbol.ClassSymbol) t.tsym).flatname.toString().replace('.', '/') + ";";
            case METHOD -> {
                StringBuilder out = new StringBuilder("(");
                for (Type param : t.getParameterTypes()) {
                    String d = descriptor(types, param);
                    if (d == null) yield null;
                    out.append(d);
                }
                String result = descriptor(types, t.getReturnType());
                yield result == null ? null : out.append(")").append(result).toString();
            }
            default -> null;
        };
    }

    // The statement around `path`, out of any lambdas it is in: the one of the method's body (or
    // the field) that holds the whole expression.
    static JCTree statementAround(TreePath path) {
        JCTree statement = null;
        for (TreePath up = path; up != null; up = up.getParentPath()) {
            Tree leaf = up.getLeaf();
            if (leaf instanceof JCTree.JCMethodDecl || leaf instanceof JCTree.JCClassDecl) break;
            if (leaf instanceof JCTree.JCStatement s && !(leaf instanceof JCTree.JCBlock)) {
                statement = s;
                if (!insideLambda(up.getParentPath())) break;
            }
        }
        return statement;
    }

    // A diamond for each `new C(..)` of a generic class written raw that is an argument anywhere
    // in the statement around the error (`traverse(new Pair.Instance(), input)` inside the
    // `dimap(..)` that javac cannot infer), the decompiler's way of writing `new C<>(..)`.
    static List<Edit> diamondsAround(JCTree.JCCompilationUnit cu, TreePath path) {
        JCTree statement = statementAround(path);
        if (statement == null) return List.of();
        List<Edit> edits = new ArrayList<>();
        new TreeScanner<Void, Void>() {
            void arguments(List<JCTree.JCExpression> args) {
                for (JCTree.JCExpression argument : args) {
                    if (!(argument instanceof JCTree.JCNewClass created) || created.typeargs.nonEmpty() || created.def != null
                            || created.clazz instanceof JCTree.JCTypeApply || created.clazz.type == null
                            || created.clazz.type.tsym.type.getTypeArguments().isEmpty()) continue;
                    int end = TreeInfo.getEndPos(created.clazz, cu.endPositions);
                    edits.add(new Edit(end, end, "<>", "diamond in an argument: " + oneLine(source(cu,
                        TreeInfo.getStartPos(created), end)) + "<>"));
                }
            }
            public Void visitMethodInvocation(MethodInvocationTree node, Void v) {
                arguments(((JCTree.JCMethodInvocation) node).args);
                return super.visitMethodInvocation(node, v);
            }
            public Void visitNewClass(NewClassTree node, Void v) {
                arguments(((JCTree.JCNewClass) node).args);
                return super.visitNewClass(node, v);
            }
        }.scan(statement, null);
        return edits;
    }

    // Where the method around the error has one type variable that nothing in it names, each `?`
    // that a cast in the statement around the error has as a type argument, written as that
    // variable (`Functions.comp((PointFree<Function<B, C>>) first, (PointFree<Function<A, B>>)
    // second)` in a `<A, B, C>` method whose `B` the decompiler wrote as `?`). Type arguments of a
    // cast change no `checkcast`.
    static List<Edit> unusedVariableWildcards(JCTree.JCCompilationUnit cu, TreePath path) {
        JCTree.JCMethodDecl method = null;
        for (TreePath up = path; up != null && method == null; up = up.getParentPath()) {
            if (up.getLeaf() instanceof JCTree.JCClassDecl) return List.of();
            if (up.getLeaf() instanceof JCTree.JCMethodDecl m) method = m;
        }
        if (method == null || method.sym == null) return List.of();
        Set<Symbol> unused = new LinkedHashSet<>(method.sym.getTypeParameters());
        new TreeScanner<Void, Void>() {
            public Void visitIdentifier(IdentifierTree node, Void v) {
                unused.remove(((JCTree.JCIdent) node).sym);
                return null;
            }
            public Void visitTypeParameter(TypeParameterTree node, Void v) {
                return scan(node.getBounds(), v);
            }
        }.scan(method, null);
        if (unused.size() != 1) return List.of();
        String name = unused.iterator().next().name.toString();
        JCTree statement = statementAround(path);
        if (statement == null) return List.of();
        List<Edit> edits = new ArrayList<>();
        new TreeScanner<Void, Void>() {
            public Void visitTypeCast(TypeCastTree node, Void v) {
                new TreeScanner<Void, Void>() {
                    public Void visitParameterizedType(ParameterizedTypeTree type, Void w) {
                        for (Tree argument : type.getTypeArguments())
                            if (argument instanceof JCTree.JCWildcard wildcard && wildcard.kind.kind == com.sun.tools.javac.code.BoundKind.UNBOUND) {
                                int start = TreeInfo.getStartPos(wildcard);
                                edits.add(new Edit(start, TreeInfo.getEndPos(wildcard, cu.endPositions), name,
                                    "the unnamed type variable " + name + " for ? in a cast"));
                            }
                        return super.visitParameterizedType(type, w);
                    }
                }.scan(node.getType(), null);
                return super.visitTypeCast(node, v);
            }
        }.scan(statement, null);
        return edits;
    }

    // The decompiler's casts that javac compiles to no `checkcast` (the operand's erasure is
    // already a subtype of the cast's), in the statement around the error, out of any lambdas it
    // is in: all left out, one option; with `arguments`, only those between parameterizations of
    // one class. A cast to a wider class stays where it can choose what the code means: one not
    // to a parameterized type (`(Object) s`), one whose member is selected (a hidden field), and
    // one passed to a method or constructor that has an overload as many parameters long. Such a cast is the decompiler's guess at a type, often
    // with the wrong type variables (`(Either<Either<B, C>, A>) prism.match(a)` for `A2`), which
    // then keeps javac from inferring what the original inferred. Casts of lambdas and method
    // references stay: they give their interface.
    static List<Edit> noOpCasts(Types types, JCTree.JCCompilationUnit cu, TreePath path, boolean arguments) {
        JCTree statement = statementAround(path);
        if (statement == null) return List.of();
        List<Edit> edits = new ArrayList<>();
        new TreePathScanner<Void, Void>() {
            public Void visitTypeCast(TypeCastTree node, Void v) {
                JCTree.JCTypeCast cast = (JCTree.JCTypeCast) node;
                JCTree.JCExpression operand = TreeInfo.skipParens(cast.expr);
                Type to = cast.clazz.type, from = cast.expr.type;
                // a type variable named out of its scope (a class's in a static member)
                if ((to == null || to.isErroneous()) && TreeInfo.symbol(cast.clazz) instanceof Symbol.TypeVariableSymbol variable)
                    to = variable.type;
                if (to != null && from != null && !to.isErroneous() && !from.isErroneous() && !to.isPrimitive()
                        && !from.isPrimitive() && !from.hasTag(TypeTag.BOT)
                        && !(operand instanceof JCTree.JCLambda || operand instanceof JCTree.JCMemberReference)
                        && types.isSubtype(types.erasure(from), types.erasure(to))
                        && (types.isSameType(types.erasure(from), types.erasure(to))
                            ? !arguments || to.hasTag(TypeTag.CLASS) && to.isParameterized()
                            : !arguments && to.hasTag(TypeTag.CLASS) && to.isParameterized()
                                && !meaningful(types, getCurrentPath()))) {
                    int start = cast.getStartPosition(), expr = cast.expr.getStartPosition();
                    edits.add(new Edit(start, expr, "", "no cast " + source(cu, start, expr).strip() + " that compiles to nothing"));
                }
                return super.visitTypeCast(node, v);
            }
        }.scan(TreePath.getPath(cu, statement), null);
        return edits;
    }

    // Whether the static type of the cast at `path` can choose what the code around it means: it
    // is the receiver of a member, or an argument of a method or constructor with another one of
    // that name and as many parameters.
    static boolean meaningful(Types types, TreePath path) {
        TreePath up = path.getParentPath();
        while (up != null && up.getLeaf() instanceof JCTree.JCParens) up = up.getParentPath();
        if (up == null) return false;
        Tree parent = up.getLeaf();
        if (parent instanceof JCTree.JCFieldAccess || parent instanceof JCTree.JCMemberReference) return true;
        Type site;
        Name name;
        int count;
        if (parent instanceof JCTree.JCMethodInvocation call) {
            // the receiver's type, as a call that javac could not resolve has no method
            Symbol called = TreeInfo.symbol(call.meth);
            if (call.meth instanceof JCTree.JCFieldAccess access) site = access.selected.type;
            else if (called != null && called.owner instanceof Symbol.ClassSymbol c) site = c.type;
            else return true;
            name = TreeInfo.name(call.meth);
            count = call.args.size();
        } else if (parent instanceof JCTree.JCNewClass created) {
            site = created.clazz.type;
            name = site == null ? null : site.tsym.name.table.names.init;
            count = created.args.size();
        } else {
            return false;
        }
        if (site == null || !site.hasTag(TypeTag.CLASS) || name == null) return true;
        // the methods of that name and length, each counted once with those it overrides
        List<Symbol.MethodSymbol> found = new ArrayList<>();
        Symbol.ClassSymbol origin = (Symbol.ClassSymbol) site.tsym;
        for (Type t : name == name.table.names.init ? List.of(site) : types.closure(site))
            for (Symbol member : t.tsym.members().getSymbolsByName(name))
                if (member instanceof Symbol.MethodSymbol o && (o.params().size() == count || o.isVarArgs())
                        && found.stream().noneMatch(f -> f.overrides(o, origin, types, false)))
                    found.add(o);
        return found.size() != 1;
    }

    // Whether a lambda is around `path` below its method or class.
    static boolean insideLambda(TreePath path) {
        for (TreePath up = path; up != null; up = up.getParentPath()) {
            if (up.getLeaf() instanceof JCTree.JCLambda) return true;
            if (up.getLeaf() instanceof JCTree.JCMethodDecl || up.getLeaf() instanceof JCTree.JCClassDecl) return false;
        }
        return false;
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
