import com.sun.source.tree.*;
import com.sun.source.util.*;
import com.sun.tools.javac.api.JavacTaskImpl;
import java.io.*;
import java.lang.reflect.Method;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.*;
import javax.lang.model.element.*;
import javax.lang.model.type.*;
import javax.lang.model.util.Elements;
import javax.lang.model.util.Types;
import javax.tools.*;

/**
 * Compiles one Java source file with javac (parse and attribute, no code generation) and prints
 * its syntax tree as JSON, with the Java type of every expression and the declaration that every
 * name refers to.
 *
 * <p>Usage: {@code java JavaTree [FileName.java] < source}. On compile errors it prints them as
 * {@code LINE:COLUMN: message} to standard error and exits with 1.
 *
 * <p>The tree is dumped generically: every node is an object with its {@code kind} (the name of
 * {@link Tree.Kind}), its position, and one field per getter of the node's interface, named after
 * the getter ({@code getLeftOperand} gives {@code leftOperand}). Expressions also get
 * {@code javaType}; names, calls, {@code new}, and declarations get {@code symbol}, and a method's
 * symbol lists the methods it {@code overrides}.
 */
public class JavaTree {
    public static void main(String[] args) throws Exception {
        if (args.length > 0 && args[0].equals("--program")) {
            program(Arrays.asList(args).subList(1, args.length));
            return;
        }
        String name = args.length > 0 ? args[0] : "Main.java";
        String source = new String(System.in.readAllBytes(), StandardCharsets.UTF_8);
        JavaFileObject file = new SimpleJavaFileObject(URI.create("string:///" + name), JavaFileObject.Kind.SOURCE) {
            @Override
            public CharSequence getCharContent(boolean ignoreEncodingErrors) {
                return source;
            }

            @Override
            public boolean isNameCompatible(String simpleName, Kind kind) {
                return true;
            }
        };
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
        JavacTask task = (JavacTask) compiler.getTask(null, null, diagnostics,
                List.of("-proc:none", "-Xlint:none", "-XDshould-stop.at=FLOW"), null, List.of(file));
        context = ((JavacTaskImpl) task).getContext();
        elements = task.getElements();
        types = task.getTypes();
        Iterable<? extends CompilationUnitTree> units = task.parse();
        task.analyze();
        boolean failed = false;
        for (Diagnostic<? extends JavaFileObject> d : diagnostics.getDiagnostics()) {
            if (d.getKind() == Diagnostic.Kind.ERROR) {
                System.err.println(d.getLineNumber() + ":" + d.getColumnNumber() + ": " + d.getMessage(Locale.ROOT));
                failed = true;
            }
        }
        if (failed) {
            System.exit(1);
        }
        CompilationUnitTree unit = units.iterator().next();
        JavaTree dumper = new JavaTree(Trees.instance(task), unit);
        StringBuilder out = new StringBuilder();
        dumper.node(new TreePath(unit), out);
        out.append('\n');
        PrintStream stdout = new PrintStream(new FileOutputStream(FileDescriptor.out), false, StandardCharsets.UTF_8);
        stdout.print(out);
        stdout.flush();
    }

    /**
     * Whole-program mode: {@code java JavaTree --program [--sourcepath PATH] [--module-info FILE]
     * [--patches DIR] FILE...} compiles all the files in one javac run, loading the other classes they use from the
     * source path, and prints {@code {"units":[{"file":...,"tree":...},...]}}: the files given, in order, then every
     * unit javac loaded from the source path and attributed. Compile errors are printed as
     * {@code FILE:LINE:COLUMN: message}.
     *
     * <p>javac attributes every unit it loads, and loads every unit their code names in turn:
     * thousands of the JDK's for a program that uses a collection. With {@code --serve} only the
     * given files are attributed at first, and the others as they are asked for (see
     * {@link #serve}).
     *
     * <p>With {@code --keep-going} (with {@code --serve}), compile errors do not stop it: each is
     * printed, and the piece of code that it is in is marked (see {@link #markErrors}), for a
     * census of code such as a decompiler's output, which can have errors.
     *
     * <p>Every root of the source path may be a module's, with a {@code module-info.java}: javac
     * would compile in the module of the first one it finds, and read the classes of other modules
     * from the JDK's class files instead of their sources on the path. So javac sees none of them,
     * except the one that {@code --module-info FILE} names (the JDK's {@code java.base}, whose
     * packages javac compiles only inside that module): all the sources are one module, that one
     * or the unnamed module (see {@link #oneModule}).
     *
     * <p>With {@code --patches DIR}, a source file of the source path that a patch in DIR names
     * is read as the patch changes it (see {@link JdkPatch}).
     */
    private static void program(List<String> args) throws Exception {
        List<String> options = new ArrayList<>(List.of("-proc:none", "-Xlint:none", "-XDshould-stop.at=FLOW"));
        List<File> files = new ArrayList<>();
        boolean serve = false;
        File moduleInfo = null;
        java.nio.file.Path patches = null;
        for (int i = 0; i < args.size(); i++) {
            if (args.get(i).equals("--sourcepath") && i + 1 < args.size()) {
                options.add("-sourcepath");
                options.add(args.get(++i));
            } else if (args.get(i).equals("--module-info") && i + 1 < args.size()) {
                moduleInfo = new File(args.get(++i));
            } else if (args.get(i).equals("--patches") && i + 1 < args.size()) {
                patches = java.nio.file.Path.of(args.get(++i));
            } else if (args.get(i).equals("--serve")) {
                serve = true;
            } else if (args.get(i).equals("--keep-going")) {
                keepGoing = true;
            } else {
                files.add(new File(args.get(i)));
            }
        }
        if (keepGoing) {
            // javac reports the first 100 errors by default, and goes on silently past the rest.
            options.addAll(List.of("-Xmaxerrs", Integer.toString(Integer.MAX_VALUE)));
        }
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
        StandardJavaFileManager fileManager = compiler.getStandardFileManager(diagnostics, Locale.ROOT, StandardCharsets.UTF_8);
        JavaFileManager manager = JdkPatch.patching(oneModule(fileManager, moduleInfo), patches);
        JavacTask task = (JavacTask) compiler.getTask(null, manager, diagnostics,
                options, null, fileManager.getJavaFileObjectsFromFiles(files));
        // Units that javac loads from the source path are attributed too, but only reported here.
        Set<CompilationUnitTree> analyzed = new LinkedHashSet<>();
        context = ((JavacTaskImpl) task).getContext();
        missing = new MissingClasses(context, task.getElements());
        task.addTaskListener(new TaskListener() {
            @Override
            public void finished(TaskEvent e) {
                if (e.getKind() == TaskEvent.Kind.ANALYZE) {
                    analyzed.add(e.getCompilationUnit());
                } else if (e.getKind() == TaskEvent.Kind.PARSE) {
                    missing.parsed(e.getCompilationUnit());
                }
            }
        });
        List<CompilationUnitTree> units = new ArrayList<>();
        elements = task.getElements();
        types = task.getTypes();
        task.parse().forEach(units::add);
        PrintStream stdout = new PrintStream(new BufferedOutputStream(new FileOutputStream(FileDescriptor.out), 1 << 16),
                false, StandardCharsets.UTF_8);
        if (serve) {
            serve((JavacTaskImpl) task, units, diagnostics, stdout);
            return;
        }
        task.analyze();
        failOnErrors(diagnostics);
        for (CompilationUnitTree unit : analyzed) {
            if (!units.contains(unit)) {
                units.add(unit);
            }
        }
        print(Trees.instance(task), units, stdout);
    }

    /**
     * The file manager that shows javac no {@code module-info.java} on the source path but
     * {@code moduleInfo}, when that is given. javac looks for one there to choose the module that
     * the sources it compiles are in (and lists none when it lists a package's sources, since its
     * name is not a class's).
     */
    private static JavaFileManager oneModule(StandardJavaFileManager fileManager, File moduleInfo) {
        URI kept = moduleInfo == null ? null : moduleInfo.getAbsoluteFile().toPath().normalize().toUri();
        return new ForwardingJavaFileManager<>(fileManager) {
            @Override
            public JavaFileObject getJavaFileForInput(Location location, String className, JavaFileObject.Kind kind)
                    throws IOException {
                if (location != StandardLocation.SOURCE_PATH || !className.equals("module-info")) {
                    return super.getJavaFileForInput(location, className, kind);
                }
                if (kept == null || kind != JavaFileObject.Kind.SOURCE) {
                    return null;
                }
                for (JavaFileObject file : super.list(location, "", Set.of(kind), false)) {
                    if (file.isNameCompatible("module-info", kind) && file.toUri().normalize().equals(kept)) {
                        return file;
                    }
                }
                return null;
            }
        };
    }

    /**
     * Serve mode: attributes the classes of the given units and prints the units as a line
     * {@code {"units":[...]}}; then reads lines of qualified class names separated by commas from
     * standard input, and answers each with a line of the same form: the units that declare those
     * classes, attributed, that it has not printed before. A class that the source path does not
     * have, or not as a source file, adds no unit. It ends at the end of the input. javac
     * attributes a unit as a whole run would, so the units are the same, and only those asked for
     * are attributed at all.
     */
    private static void serve(JavacTaskImpl task, List<CompilationUnitTree> given,
            DiagnosticCollector<JavaFileObject> diagnostics, PrintStream stdout) throws IOException {
        Trees trees = Trees.instance(task);
        // The given units' classes have symbols once entered.
        task.enter();
        Set<CompilationUnitTree> printed = new HashSet<>(given);
        List<CompilationUnitTree> units = given;
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        while (true) {
            List<Element> classes = new ArrayList<>();
            for (CompilationUnitTree unit : units) {
                for (Tree decl : unit.getTypeDecls()) {
                    if (trees.getElement(new TreePath(new TreePath(unit), decl)) instanceof TypeElement t) {
                        classes.add(t);
                    }
                }
            }
            task.analyze(classes);
            if (keepGoing) {
                takeErrors(diagnostics);
            } else {
                failOnErrors(diagnostics);
            }
            print(trees, units, stdout);
            for (CompilationUnitTree unit : units) {
                release(unit);
            }
            String line = in.readLine();
            if (line == null) {
                return;
            }
            units = new ArrayList<>();
            for (String name : line.split(",")) {
                TypeElement type = name.isEmpty() ? null : elements.getTypeElement(name);
                TreePath path = type == null ? null : trees.getPath(outermost(type));
                if (path != null && printed.add(path.getCompilationUnit())) {
                    units.add(path.getCompilationUnit());
                }
            }
        }
    }

    /** The outermost class that declares a class, or the class itself. */
    private static TypeElement outermost(TypeElement type) {
        TypeElement top = type;
        for (Element at = type; at != null && !(at instanceof PackageElement); at = at.getEnclosingElement()) {
            if (at instanceof TypeElement t) {
                top = t;
            }
        }
        return top;
    }

    /**
     * Prints the units as a line {@code {"units":[{"file":...,"tree":...},...]}}, each unit as soon
     * as its text is made, so that the text of only one is held at a time.
     */
    private static void print(Trees trees, List<CompilationUnitTree> units, PrintStream stdout) {
        stdout.print("{\"units\":[");
        StringBuilder out = new StringBuilder();
        for (CompilationUnitTree unit : units) {
            out.setLength(0);
            if (unit != units.get(0)) {
                out.append(',');
            }
            out.append("{\"file\":");
            string(unit.getSourceFile().getName(), out);
            out.append(",\"tree\":");
            JavaTree printer = new JavaTree(trees, unit);
            printer.markErrors(errors.remove(unit.getSourceFile().toUri()));
            printer.node(new TreePath(unit), out);
            out.append('}');
            stdout.append(out);
            // A large unit's text is not kept for the next.
            if (out.capacity() > 1 << 20) {
                out = new StringBuilder();
            }
        }
        stdout.print("]}\n");
        stdout.flush();
    }

    /**
     * Lets javac free the method bodies of a unit that has been printed: in serve mode javac
     * attributes more units afterwards, which need the unit's classes and their members, but
     * not the code of its methods. Field initializers stay, since a constant's value may be
     * computed from one when another unit uses it.
     */
    private static void release(CompilationUnitTree unit) {
        new TreeScanner<Void, Void>() {
            @Override
            public Void visitMethod(MethodTree method, Void unused) {
                ((com.sun.tools.javac.tree.JCTree.JCMethodDecl) method).body = null;
                return null;
            }
        }.scan(unit, null);
    }

    /**
     * Prints the compile errors that javac reported since the last call as
     * {@code FILE:LINE:COLUMN: message} and exits, if there are any, except those that name a
     * missing class where the JVM has no counterpart of the error ({@link MissingClasses#ignored}).
     * Each is classified when it is reported, while its unit's code is all there.
     */
    private static void failOnErrors(DiagnosticCollector<JavaFileObject> diagnostics) {
        boolean failed = false;
        List<Diagnostic<? extends JavaFileObject>> all = diagnostics.getDiagnostics();
        for (; errorsTaken < all.size(); errorsTaken++) {
            Diagnostic<? extends JavaFileObject> d = all.get(errorsTaken);
            MissingClasses.Missing named = d.getKind() == Diagnostic.Kind.ERROR ? missing.classify(d) : null;
            if (d.getKind() == Diagnostic.Kind.ERROR && (named == null || !missing.ignored(named))) {
                String file = d.getSource() == null ? "" : d.getSource().getName();
                System.err.println(file + ":" + d.getLineNumber() + ":" + d.getColumnNumber() + ": " + d.getMessage(Locale.ROOT));
                failed = true;
            }
        }
        if (failed) {
            System.exit(1);
        }
    }

    /**
     * Prints the compile errors that javac reported since the last call, as
     * {@code javac error: FILE:LINE:COLUMN: CODE: message}, and keeps those with a place in a file
     * for when the file's unit is printed ({@link #markErrors}), each with the missing class it
     * names, if it names one ({@link MissingClasses#classify}). An error of a unit is reported
     * when javac parses or attributes it, before it is printed.
     */
    private static void takeErrors(DiagnosticCollector<JavaFileObject> diagnostics) {
        List<Diagnostic<? extends JavaFileObject>> all = diagnostics.getDiagnostics();
        for (; errorsTaken < all.size(); errorsTaken++) {
            Diagnostic<? extends JavaFileObject> d = all.get(errorsTaken);
            if (d.getKind() != Diagnostic.Kind.ERROR) {
                continue;
            }
            MissingClasses.Missing named = missing.classify(d);
            if (named != null && missing.ignored(named)) {
                continue;
            }
            String file = d.getSource() == null ? "" : d.getSource().getName();
            String message = d.getMessage(Locale.ROOT).lines().findFirst().orElse("");
            System.err.println("javac error: " + file + ":" + d.getLineNumber() + ":" + d.getColumnNumber() + ": "
                    + d.getCode() + ": " + message);
            if (d.getSource() != null && d.getPosition() != Diagnostic.NOPOS) {
                errors.computeIfAbsent(d.getSource().toUri(), f -> new ArrayList<>()).add(new Found(d, named));
            }
        }
        System.err.flush();
    }

    /** {@code --keep-going}: compile errors are marked in the trees instead of stopping the run. */
    private static boolean keepGoing;
    /**
     * The compile errors that name missing classes: those that the JVM has no counterpart of are
     * dropped, and with {@code --keep-going} the code that names one throws as the JVM would.
     */
    private static MissingClasses missing;
    /** How many of the diagnostics {@link #takeErrors} or {@link #failOnErrors} has seen. */
    private static int errorsTaken;
    /** A compile error, with the missing class it names, if it names one. */
    private record Found(Diagnostic<? extends JavaFileObject> diagnostic, MissingClasses.Missing missing) {}
    /** The compile errors of the units not printed yet, by file. */
    private static final Map<URI, List<Found>> errors = new HashMap<>();

    private static Elements elements;
    private static Types types;
    /** javac's context. */
    private static com.sun.tools.javac.util.Context context;

    private final Trees trees;
    private final CompilationUnitTree unit;
    private final SourcePositions positions;
    /** The pieces of code that compile errors are in, with the code of the first error of each. */
    private final Map<Tree, String> broken = new IdentityHashMap<>();
    /** The pieces of code that throw {@code NoClassDefFoundError}, with the missing class each names first. */
    private Map<Tree, String> missingPieces = Map.of();

    private JavaTree(Trees trees, CompilationUnitTree unit) {
        this.trees = trees;
        this.unit = unit;
        this.positions = trees.getSourcePositions();
    }

    /**
     * Marks the piece of code that each error is in, which the census rejects for it: the
     * innermost method, initializer block of a class, initializer of a field or, for an error in
     * none of those, such as one in a class's {@code extends}, class. A lambda between the error
     * and that piece is marked too, as code of its own. The trees print the mark as
     * {@code "javacError": CODE}, with javac's code of the error (such as
     * {@code compiler.err.cant.resolve.location}). An error outside every class, such as one of an
     * import, marks nothing: the code that uses what it names has errors of its own. Nor does an
     * error in an annotation, such as one whose class is missing (a decompiled library's
     * compile-only annotations, which the translation leaves out anyway). Nor does an error that
     * names a missing class, or that only missing classes cause: the code that does something
     * with the class is rewritten instead ({@link MissingClasses}).
     */
    private void markErrors(List<Found> found) {
        List<Diagnostic<? extends JavaFileObject>> errors = new ArrayList<>();
        MissingClasses.Plan plan = null;
        if (keepGoing) {
            List<MissingClasses.Missing> missingErrors = new ArrayList<>();
            for (Found f : found == null ? List.<Found>of() : found) {
                if (f.missing() != null) {
                    missingErrors.add(f.missing());
                }
            }
            plan = missing.plan((com.sun.tools.javac.tree.JCTree.JCCompilationUnit) unit, missingErrors);
        }
        for (Found f : found == null ? List.<Found>of() : found) {
            if (f.missing() == null && (plan == null
                    || !missing.explains(plan, f.diagnostic(), (com.sun.tools.javac.tree.JCTree.JCCompilationUnit) unit))) {
                errors.add(f.diagnostic());
            }
        }
        markBroken(errors);
        if (plan != null) {
            missingPieces = missing.rewrite(plan, broken.keySet());
        }
    }

    /** Marks the pieces of code that {@code errors} are in, in {@link #broken}. */
    private void markBroken(List<Diagnostic<? extends JavaFileObject>> errors) {
        if (errors.isEmpty()) {
            return;
        }
        List<Tree> pieces = new ArrayList<>();
        // The field initializers, which are pieces of their own when they are lambdas too.
        Set<Tree> initializers = Collections.newSetFromMap(new IdentityHashMap<>());
        List<Tree> annotations = new ArrayList<>();
        new TreePathScanner<Void, Void>() {
            private boolean member() {
                return getCurrentPath().getParentPath().getLeaf() instanceof ClassTree;
            }

            @Override
            public Void visitAnnotation(AnnotationTree tree, Void unused) {
                annotations.add(tree);
                return null;
            }

            @Override
            public Void visitClass(ClassTree tree, Void unused) {
                pieces.add(tree);
                return super.visitClass(tree, unused);
            }

            @Override
            public Void visitMethod(MethodTree tree, Void unused) {
                pieces.add(tree);
                return super.visitMethod(tree, unused);
            }

            @Override
            public Void visitLambdaExpression(LambdaExpressionTree tree, Void unused) {
                pieces.add(tree);
                return super.visitLambdaExpression(tree, unused);
            }

            @Override
            public Void visitBlock(BlockTree tree, Void unused) {
                if (member()) {
                    pieces.add(tree);
                }
                return super.visitBlock(tree, unused);
            }

            @Override
            public Void visitVariable(VariableTree tree, Void unused) {
                if (member() && tree.getInitializer() != null) {
                    pieces.add(tree.getInitializer());
                    initializers.add(tree.getInitializer());
                }
                return super.visitVariable(tree, unused);
            }
        }.scan(unit, null);
        for (Diagnostic<? extends JavaFileObject> error : errors) {
            long at = error.getPosition();
            if (annotations.stream().anyMatch(a -> within(a, at))) {
                continue;
            }
            List<Tree> around = new ArrayList<>();
            for (Tree piece : pieces) {
                if (within(piece, at)) {
                    around.add(piece);
                }
            }
            // From the innermost piece out, up to the first that is not a lambda.
            around.sort(Comparator.comparingLong(p -> positions.getEndPosition(unit, p) == Diagnostic.NOPOS
                    ? Long.MAX_VALUE
                    : positions.getEndPosition(unit, p) - positions.getStartPosition(unit, p)));
            for (Tree piece : around) {
                broken.putIfAbsent(piece, error.getCode());
                if (!(piece instanceof LambdaExpressionTree) || initializers.contains(piece)) {
                    break;
                }
            }
        }
    }

    /** Whether the position {@code at} is in the text of {@code tree}. */
    private boolean within(Tree tree, long at) {
        long start = positions.getStartPosition(unit, tree);
        long end = positions.getEndPosition(unit, tree);
        return start != Diagnostic.NOPOS && start <= at && (end == Diagnostic.NOPOS || at <= end);
    }

    /** Getters of each node interface, in declaration order, without those of Tree itself. */
    private static final Map<Class<?>, List<Method>> GETTERS = new HashMap<>();

    private static List<Method> getters(Class<?> type) {
        return GETTERS.computeIfAbsent(type, t -> {
            List<Method> out = new ArrayList<>();
            Set<String> seen = new HashSet<>();
            for (Method m : t.getMethods()) {
                if (m.getParameterCount() == 0 && m.getDeclaringClass() != Tree.class
                        && m.getDeclaringClass() != Object.class
                        && (m.getName().startsWith("get") || m.getName().startsWith("is"))
                        && seen.add(m.getName())) {
                    out.add(m);
                }
            }
            out.sort(Comparator.comparing(Method::getName));
            return out;
        });
    }

    private static String field(Method m) {
        String n = m.getName().startsWith("get") ? m.getName().substring(3) : m.getName().substring(2);
        return Character.toLowerCase(n.charAt(0)) + n.substring(1);
    }

    private void node(TreePath path, StringBuilder out) {
        Tree tree = path.getLeaf();
        out.append("{\"kind\":");
        string(tree.getKind().name(), out);
        long start = positions.getStartPosition(unit, tree);
        if (start >= 0) {
            out.append(",\"line\":").append(unit.getLineMap().getLineNumber(start));
            out.append(",\"column\":").append(unit.getLineMap().getColumnNumber(start));
        }
        String error = broken.get(tree);
        if (error != null) {
            out.append(",\"javacError\":");
            string(error, out);
        }
        String missingClass = missingPieces.get(tree);
        if (missingClass != null) {
            out.append(",\"missingClass\":");
            string(missingClass, out);
        }
        if (tree instanceof ExpressionTree) {
            TypeMirror type = recovered(tree, trees.getTypeMirror(path));
            if (type != null) {
                out.append(",\"javaType\":");
                string(typeName(type), out);
            }
            if (type != null && (tree instanceof LambdaExpressionTree || tree instanceof MemberReferenceTree)) {
                descriptor(type, tree, path, out);
            }
            if (tree instanceof TypeCastTree cast && cast.getType() instanceof IntersectionTypeTree) {
                TypeMirror expected = expected(path);
                if (expected != null) {
                    out.append(",\"expected\":");
                    string(typeText(expected), out);
                }
            }
        }
        if (tree instanceof IdentifierTree || tree instanceof MemberSelectTree || tree instanceof MethodInvocationTree
                || tree instanceof NewClassTree || tree instanceof VariableTree || tree instanceof MethodTree
                || tree instanceof ClassTree || tree instanceof MemberReferenceTree) {
            Element element = trees.getElement(path);
            if (element != null) {
                out.append(",\"symbol\":");
                symbol(element, tree instanceof MethodTree || tree instanceof ClassTree, out);
            }
        }
        Class<?> type = tree.getKind().asInterface();
        for (Method m : getters(type)) {
            Object value;
            try {
                value = m.invoke(tree);
            } catch (ReflectiveOperationException e) {
                continue;
            }
            int mark = out.length();
            out.append(",\"").append(field(m)).append("\":");
            if (!value(path, value, out)) {
                out.setLength(mark);
            }
        }
        out.append('}');
    }

    /** Appends a getter's value; returns false (appending nothing) for values that are skipped. */
    private boolean value(TreePath path, Object value, StringBuilder out) {
        if (value == null) {
            out.append("null");
        } else if (value instanceof Tree t) {
            node(new TreePath(path, t), out);
        } else if (value instanceof Collection<?> items) {
            out.append('[');
            boolean first = true;
            for (Object item : items) {
                if (!first) {
                    out.append(',');
                }
                first = false;
                if (!value(path, item, out)) {
                    out.append("null");
                }
            }
            out.append(']');
        } else if (value instanceof Boolean b) {
            out.append(b);
        } else if (value instanceof Enum<?> e) {
            string(e.name(), out);
        } else if (value instanceof Character c) {
            // A char literal as its UTF-16 code unit: JSON readers reject a lone surrogate.
            string(Integer.toString(c), out);
        } else if (value instanceof Number || value instanceof CharSequence) {
            // Literal values and names; numbers as text so that no precision is lost.
            string(value.toString(), out);
        } else {
            return false;
        }
        return true;
    }

    private void symbol(Element e, boolean declaration, StringBuilder out) {
        out.append("{\"kind\":");
        string(e.getKind().name(), out);
        out.append(",\"name\":");
        string(e.getSimpleName().toString(), out);
        Element owner = e.getEnclosingElement();
        if (owner != null) {
            out.append(",\"owner\":");
            // javac names an initializer block after its class; a type declared in one is owned
            // by `{}`, or `static {}`.
            boolean block = owner.getKind() == ElementKind.INSTANCE_INIT || owner.getKind() == ElementKind.STATIC_INIT;
            string(owner instanceof TypeElement t ? className(t)
                    : block && e instanceof TypeElement ? (owner.getKind() == ElementKind.STATIC_INIT ? "static {}" : "{}")
                    : owner.toString(), out);
        }
        out.append(",\"static\":").append(e.getModifiers().contains(Modifier.STATIC));
        // javac prints a varargs parameter's type as an array, so only this says it is varargs.
        if (e instanceof ExecutableElement m && m.isVarArgs()) {
            out.append(",\"varargs\":true");
        }
        // A native method has no body: the JVM implements it, and so must the runtime.
        if (e.getModifiers().contains(Modifier.NATIVE)) {
            out.append(",\"native\":true");
        }
        out.append(",\"type\":");
        string(typeText(e.asType()), out);
        if (declaration && e.getKind() == ElementKind.METHOD && owner instanceof TypeElement t) {
            overrides((ExecutableElement) e, t, out);
        }
        if (declaration && e instanceof TypeElement t && (e.getKind() == ElementKind.CLASS || e.getKind() == ElementKind.ENUM)) {
            inherits(t, out);
        }
        // An anonymous class's binary name, which the class file names its methods by.
        if (declaration && e instanceof TypeElement t && t.getNestingKind() == NestingKind.ANONYMOUS) {
            out.append(",\"binary\":");
            string(elements.getBinaryName(t).toString(), out);
        }
        // The value of a constant variable, as text: javac's compile-time constant.
        if (e instanceof VariableElement v && v.getConstantValue() != null) {
            Object c = v.getConstantValue();
            out.append(",\"constant\":");
            string(c instanceof Character ch ? Integer.toString(ch) : c.toString(), out);
        }
        out.append('}');
    }

    /**
     * The type of an expression that javac left as an error type, with {@code --keep-going}, though
     * its value is not of a missing class ({@link MissingClasses}): the erasure of the class that
     * {@code new} creates or that a parameterized type names, or of the result of the method
     * called, when inferring type arguments from a missing class failed ({@code new Box<>()} for a
     * {@code Box<Missing>}); {@code boolean} for a comparison or a logical operator, and
     * {@code String} for a {@code +} on a string, with an operand of a missing class.
     */
    private static TypeMirror recovered(Tree tree, TypeMirror type) {
        if (!keepGoing || type == null || type.getKind() != TypeKind.ERROR) {
            return type;
        }
        com.sun.tools.javac.code.Type found = recovered((com.sun.tools.javac.tree.JCTree) tree);
        return found == null ? type : found;
    }

    private static com.sun.tools.javac.code.Type recovered(com.sun.tools.javac.tree.JCTree tree) {
        com.sun.tools.javac.code.Type type = tree.type;
        if (type != null && !type.hasTag(com.sun.tools.javac.code.TypeTag.ERROR)) {
            return type;
        }
        com.sun.tools.javac.code.Symtab syms = com.sun.tools.javac.code.Symtab.instance(context);
        com.sun.tools.javac.code.Symbol named = null;
        switch (tree) {
            case com.sun.tools.javac.tree.JCTree.JCNewClass n -> named = com.sun.tools.javac.tree.TreeInfo.symbol(n.clazz);
            case com.sun.tools.javac.tree.JCTree.JCTypeApply a -> named = com.sun.tools.javac.tree.TreeInfo.symbol(a.clazz);
            case com.sun.tools.javac.tree.JCTree.JCMethodInvocation m -> named = com.sun.tools.javac.tree.TreeInfo.symbol(m.meth);
            case com.sun.tools.javac.tree.JCTree.JCParens p -> {
                return recovered(p.expr);
            }
            case com.sun.tools.javac.tree.JCTree.JCInstanceOf i -> {
                return syms.booleanType;
            }
            case com.sun.tools.javac.tree.JCTree.JCUnary u when u.hasTag(com.sun.tools.javac.tree.JCTree.Tag.NOT) -> {
                return syms.booleanType;
            }
            case com.sun.tools.javac.tree.JCTree.JCBinary b -> {
                switch (b.getTag()) {
                    case EQ, NE, LT, GT, LE, GE, AND, OR -> {
                        return syms.booleanType;
                    }
                    case PLUS -> {
                        com.sun.tools.javac.code.Type left = recovered(b.lhs);
                        com.sun.tools.javac.code.Type right = recovered(b.rhs);
                        boolean text = left != null && left.tsym == syms.stringType.tsym
                                || right != null && right.tsym == syms.stringType.tsym;
                        return text ? syms.stringType : null;
                    }
                    default -> {
                        return null;
                    }
                }
            }
            default -> {}
        }
        if (named instanceof com.sun.tools.javac.code.Symbol.ClassSymbol c && !c.type.isErroneous()) {
            return (com.sun.tools.javac.code.Type) types.erasure(c.type);
        }
        if (named instanceof com.sun.tools.javac.code.Symbol.MethodSymbol m && !m.getReturnType().isErroneous()) {
            return (com.sun.tools.javac.code.Type) types.erasure(m.getReturnType());
        }
        return null;
    }

    /**
     * The type that javac's erasure ({@code TransTypes}) translates a cast in {@code path} for, its
     * {@code pt}: the erasure of the variable it initializes or is assigned to, of the parameter
     * it is an argument for (as the call instantiates it, or the varargs element), of the
     * operand of the operator it is an operand of, of the method or lambda it returns from, of
     * the element of the array it initializes, of the {@code ?:} it is a branch of, or of the
     * cast or the receiver it is in; its own erasure where it is a {@code synchronized} lock, a
     * thrown value, a switch's selector or result, a for-each's iterable or an assert's detail.
     * javac casts a value to an intersection's bounds but that one, then to its erasure, and the
     * place converts it to that one last. {@code null} for other places: javac itself fails on a
     * cast to an intersection that is an {@code instanceof}'s value or a {@code void} lambda's.
     */
    private TypeMirror expected(TreePath path) {
        TreePath inner = path;
        while (inner.getParentPath().getLeaf() instanceof ParenthesizedTree) {
            inner = inner.getParentPath();
        }
        Tree child = inner.getLeaf();
        TreePath parent = inner.getParentPath();
        TypeMirror own = trees.getTypeMirror(path);
        TypeMirror type = switch (parent.getLeaf()) {
            case VariableTree v when v.getInitializer() == child -> trees.getTypeMirror(parent);
            case AssignmentTree a when a.getExpression() == child -> trees.getTypeMirror(new TreePath(parent, a.getVariable()));
            case CompoundAssignmentTree a when a.getExpression() == child -> operand(a, 1);
            case BinaryTree b -> operand(b, b.getLeftOperand() == child ? 0 : 1);
            case UnaryTree u -> operand(u, 0);
            case ConditionalExpressionTree c when c.getCondition() != child -> trees.getTypeMirror(parent);
            case ConditionalExpressionTree c -> types.getPrimitiveType(TypeKind.BOOLEAN);
            case IfTree i -> types.getPrimitiveType(TypeKind.BOOLEAN);
            case WhileLoopTree w -> types.getPrimitiveType(TypeKind.BOOLEAN);
            case DoWhileLoopTree d when d.getCondition() == child -> types.getPrimitiveType(TypeKind.BOOLEAN);
            case ForLoopTree f when f.getCondition() == child -> types.getPrimitiveType(TypeKind.BOOLEAN);
            case AssertTree a when a.getCondition() == child -> types.getPrimitiveType(TypeKind.BOOLEAN);
            case ArrayAccessTree a when a.getIndex() == child -> types.getPrimitiveType(TypeKind.INT);
            case TypeCastTree c -> trees.getTypeMirror(parent);
            case MemberSelectTree s -> own;
            case SynchronizedTree s -> own;
            case ThrowTree t -> own;
            case YieldTree y -> own;
            case SwitchTree s -> own;
            case SwitchExpressionTree s -> own;
            // A rule's expression in a switch expression is the value it yields.
            case CaseTree c when c.getBody() == child -> own;
            case EnhancedForLoopTree f when f.getExpression() == child -> own;
            case AssertTree a when a.getDetail() == child -> own;
            case NewArrayTree n when n.getInitializers() != null && n.getInitializers().contains(child) ->
                    trees.getTypeMirror(parent) instanceof ArrayType array ? array.getComponentType() : null;
            case MethodInvocationTree m -> {
                var call = (com.sun.tools.javac.tree.JCTree.JCMethodInvocation) m;
                yield argument(call.meth.type, call.varargsElement, m.getArguments().indexOf(child), false);
            }
            case NewClassTree n -> {
                var call = (com.sun.tools.javac.tree.JCTree.JCNewClass) n;
                yield argument(call.constructorType, call.varargsElement, n.getArguments().indexOf(child), true);
            }
            case ReturnTree r -> {
                TreePath at = parent;
                while (at != null && !(at.getLeaf() instanceof MethodTree) && !(at.getLeaf() instanceof LambdaExpressionTree)) {
                    at = at.getParentPath();
                }
                if (at != null && at.getLeaf() instanceof LambdaExpressionTree lambda) {
                    yield lambdaReturn(lambda);
                }
                yield at != null && trees.getElement(at) instanceof ExecutableElement method ? method.getReturnType() : null;
            }
            case LambdaExpressionTree l when l.getBody() == child -> lambdaReturn(l);
            default -> null;
        };
        return type == null ? null : types.erasure(type);
    }

    /** The type of operand {@code index} of the operator that javac chose for an operation. */
    private static TypeMirror operand(Tree operation, int index) {
        var operator = ((com.sun.tools.javac.tree.JCTree.JCOperatorExpression) operation).operator;
        return operator == null ? null : operator.type.getParameterTypes().get(index);
    }

    /**
     * The type that javac converts argument {@code index} of a call to: the parameter's type as
     * the call instantiates it ({@code method}), or the varargs element ({@code varargs}) for the
     * arguments that a varargs call packs. A method's parameter that is an intersection or a
     * type variable is no bound's type, so it is {@code Object} here; a constructor's is erased.
     */
    private static TypeMirror argument(com.sun.tools.javac.code.Type method, com.sun.tools.javac.code.Type varargs,
            int index, boolean constructor) {
        if (method == null || index < 0) {
            return null;
        }
        var params = method.getParameterTypes();
        com.sun.tools.javac.code.Type param = varargs != null && index >= params.size() - 1 ? varargs
                : index < params.size() ? params.get(index) : null;
        if (param != null && !constructor
                && (param.isIntersection() || param.hasTag(com.sun.tools.javac.code.TypeTag.TYPEVAR))) {
            return elements.getTypeElement("java.lang.Object").asType();
        }
        return param;
    }

    /** The return type of a lambda's function, erased; {@code null} for a {@code void} one. */
    private static TypeMirror lambdaReturn(LambdaExpressionTree lambda) {
        var javacTypes = com.sun.tools.javac.code.Types.instance(context);
        var descriptor = ((com.sun.tools.javac.tree.JCTree.JCLambda) lambda).getDescriptorType(javacTypes);
        TypeMirror result = javacTypes.erasure(descriptor).getReturnType();
        return result.getKind() == TypeKind.VOID ? null : result;
    }

    /** Interfaces that declare no methods, which an intersection type adds nothing with. */
    private static final Set<String> MARKERS = Set.of("java.io.Serializable", "java.lang.Cloneable", "java.util.RandomAccess");

    /**
     * A type as {@link #typeText} prints it, except that an intersection type ({@code (Comparator<T> &
     * Serializable)}) loses {@code Object} and the marker interfaces, and is the one type left
     * when only one is.
     */
    private static String typeName(TypeMirror type) {
        if (type instanceof IntersectionType intersection) {
            List<String> kept = new ArrayList<>();
            for (TypeMirror bound : intersection.getBounds()) {
                String name = typeText(bound);
                String raw = name.contains("<") ? name.substring(0, name.indexOf('<')) : name;
                if (!raw.equals("java.lang.Object") && !MARKERS.contains(raw)) {
                    kept.add(name);
                }
            }
            if (kept.size() == 1) {
                return kept.get(0);
            }
        }
        // A capture of `?` or `? super T` is bounded by its type variable's bound, as
        // `ChronoLocalDateTime<?>.toLocalDate()` is a `ChronoLocalDate`: written as a capture of
        // `? extends` that bound's erasure, which is the capture's erasure.
        String text = typeText(type);
        if (type instanceof TypeVariable variable && text.startsWith("capture#") && !text.contains(" of ? extends ")) {
            String bound = typeText(types.erasure(variable.getUpperBound()));
            if (!bound.equals("java.lang.Object")) {
                return text.substring(0, text.indexOf(" of ")) + " of ? extends " + bound;
            }
        }
        return text;
    }

    /**
     * The method that a lambda or method reference implements: the one abstract method of its
     * functional interface that is not one of {@code Object}'s, with the interface that declares
     * it ({@code owner}), the interface the lambda converts to ({@code target}), its declared
     * type ({@code type}, as in the method's symbol), its erasure, and its type as a member of the
     * lambda's type ({@code substituted}, with the type arguments javac inferred). A method
     * reference also gets {@code referent}, the referenced method's type as a member of its
     * qualifier's type, with its own type variables replaced by the descriptor's types.
     */
    private void descriptor(TypeMirror type, Tree tree, TreePath path, StringBuilder out) {
        if (type instanceof IntersectionType intersection) {
            for (TypeMirror bound : intersection.getBounds()) {
                String name = bound.toString();
                String raw = name.contains("<") ? name.substring(0, name.indexOf('<')) : name;
                if (!raw.equals("java.lang.Object") && !MARKERS.contains(raw)) {
                    type = bound;
                }
            }
        }
        if (!(type instanceof DeclaredType target) || !(target.asElement() instanceof TypeElement iface)) {
            return;
        }
        ExecutableElement method = null;
        for (Element member : elements.getAllMembers(iface)) {
            if (member instanceof ExecutableElement m && m.getKind() == ElementKind.METHOD
                    && m.getModifiers().contains(Modifier.ABSTRACT) && !objectMethod(m)) {
                method = m;
            }
        }
        if (method == null) {
            return;
        }
        ExecutableType substituted = (ExecutableType) types.asMemberOf(target, method);
        out.append(",\"descriptor\":{\"owner\":");
        string(className((TypeElement) method.getEnclosingElement()), out);
        out.append(",\"target\":");
        string(className(iface), out);
        out.append(",\"name\":");
        string(method.getSimpleName().toString(), out);
        out.append(",\"type\":");
        string(typeText(method.asType()), out);
        out.append(",\"erased\":");
        string(typeText(types.erasure(method.asType())), out);
        out.append(",\"substituted\":");
        string(typeText(substituted), out);
        if (tree instanceof MemberReferenceTree reference
                && trees.getElement(path) instanceof ExecutableElement referent) {
            out.append(",\"referent\":");
            string(referent(reference, referent, substituted, path), out);
        }
        out.append('}');
    }

    /**
     * The type of the method a reference names as a member of its qualifier's type
     * ({@code List<String>::get} is {@code (int)java.lang.String}); when the method has type
     * variables of its own, which javac's public API does not instantiate, its return type is the
     * descriptor's (or its erasure, for a descriptor that returns nothing).
     */
    private String referent(MemberReferenceTree reference, ExecutableElement referent, ExecutableType descriptor, TreePath path) {
        TypeMirror qualifier = trees.getTypeMirror(new TreePath(path, reference.getQualifierExpression()));
        TypeMirror member = referent.asType();
        if (qualifier instanceof DeclaredType declared && !referent.getModifiers().contains(Modifier.STATIC)) {
            try {
                member = types.asMemberOf(declared, referent);
            } catch (IllegalArgumentException e) {
                // A raw or unrelated qualifier: the declared type stands.
            }
        }
        if (referent.getTypeParameters().isEmpty() || !(member instanceof ExecutableType executable)) {
            return typeText(member);
        }
        TypeMirror ret = descriptor.getReturnType().getKind() == javax.lang.model.type.TypeKind.VOID
                ? types.erasure(executable.getReturnType()) : descriptor.getReturnType();
        StringBuilder text = new StringBuilder("(");
        for (int i = 0; i < executable.getParameterTypes().size(); i++) {
            text.append(i == 0 ? "" : ",").append(typeText(types.erasure(executable.getParameterTypes().get(i))));
        }
        return text.append(')').append(typeText(ret)).toString();
    }

    /** Whether an abstract method of an interface is one of {@code Object}'s public methods. */
    private static boolean objectMethod(ExecutableElement m) {
        String name = m.getSimpleName().toString();
        List<? extends VariableElement> params = m.getParameters();
        return switch (name) {
            case "equals" -> params.size() == 1 && types.erasure(params.get(0).asType()).toString().equals("java.lang.Object");
            case "hashCode", "toString" -> params.isEmpty();
            default -> false;
        };
    }

    /** The methods of the supertypes of {@code owner} that {@code m} overrides. */
    private void overrides(ExecutableElement m, TypeElement owner, StringBuilder out) {
        List<TypeMirror> work = new ArrayList<>(types.directSupertypes(owner.asType()));
        Set<Element> seen = new HashSet<>();
        String mark = ",\"overrides\":[";
        boolean any = false;
        while (!work.isEmpty()) {
            Element type = types.asElement(work.remove(work.size() - 1));
            if (!(type instanceof TypeElement superclass) || !seen.add(superclass)) {
                continue;
            }
            work.addAll(types.directSupertypes(superclass.asType()));
            for (Element member : superclass.getEnclosedElements()) {
                if (member instanceof ExecutableElement x && member.getKind() == ElementKind.METHOD
                        && x.getSimpleName().equals(m.getSimpleName()) && elements.overrides(m, x, owner)) {
                    out.append(any ? "," : mark);
                    any = true;
                    out.append("{\"owner\":");
                    string(className(superclass), out);
                    out.append(",\"type\":");
                    string(typeText(x.asType()), out);
                    out.append('}');
                }
            }
        }
        if (any) {
            out.append(']');
        }
    }

    /**
     * The methods that class {@code c} inherits from a superclass that implement a method of an
     * interface of {@code c} which that superclass does not implement: javac lists such an
     * implementation with neither method, as only {@code c} makes the one implement the other.
     */
    private void inherits(TypeElement c, StringBuilder out) {
        List<TypeElement> interfaces = new ArrayList<>();
        List<TypeMirror> work = new ArrayList<>(types.directSupertypes(c.asType()));
        while (!work.isEmpty()) {
            TypeMirror next = work.remove(work.size() - 1);
            if (types.asElement(next) instanceof TypeElement i && !interfaces.contains(i)) {
                if (i.getKind().isInterface()) {
                    interfaces.add(i);
                }
                work.addAll(types.directSupertypes(i.asType()));
            }
        }
        String mark = ",\"inherits\":[";
        boolean any = false;
        for (TypeMirror up = c.getSuperclass(); types.asElement(up) instanceof TypeElement s; up = s.getSuperclass()) {
            if (s.getQualifiedName().contentEquals("java.lang.Object")) {
                break;
            }
            for (Element member : s.getEnclosedElements()) {
                if (!(member instanceof ExecutableElement m) || member.getKind() != ElementKind.METHOD
                        || m.getModifiers().contains(Modifier.STATIC) || m.getModifiers().contains(Modifier.PRIVATE)) {
                    continue;
                }
                for (TypeElement i : interfaces) {
                    if (types.isSubtype(types.erasure(s.asType()), types.erasure(i.asType()))) {
                        continue;
                    }
                    for (Element candidate : i.getEnclosedElements()) {
                        if (candidate instanceof ExecutableElement x && candidate.getKind() == ElementKind.METHOD
                                && !x.getModifiers().contains(Modifier.STATIC)
                                && x.getSimpleName().equals(m.getSimpleName()) && elements.overrides(m, x, c)) {
                            out.append(any ? "," : mark);
                            any = true;
                            out.append("{\"name\":");
                            string(m.getSimpleName().toString(), out);
                            out.append(",\"owner\":");
                            string(className(s), out);
                            out.append(",\"type\":");
                            string(typeText(m.asType()), out);
                            out.append(",\"implements\":{\"owner\":");
                            string(className(i), out);
                            out.append(",\"type\":");
                            string(typeText(x.asType()), out);
                            out.append("}}");
                        }
                    }
                }
            }
        }
        if (any) {
            out.append(']');
        }
    }

    /**
     * Whether a class is local or anonymous, or nested in one. javac names such a class by its
     * simple name, or not at all, so the tree names it by its binary name: {@code Main$1Labeler},
     * {@code Main$1}. The body of an enum constant keeps javac's names.
     */
    private static boolean inLocal(Element e) {
        for (Element at = e; at instanceof TypeElement t; at = t.getEnclosingElement()) {
            boolean constantBody = t.getSuperclass() instanceof DeclaredType d && d.asElement().getKind() == ElementKind.ENUM;
            if (t.getNestingKind() == NestingKind.LOCAL || t.getNestingKind() == NestingKind.ANONYMOUS && !constantBody) {
                return true;
            }
        }
        return false;
    }

    /** The qualified name of a class, or the binary name of a local or anonymous one. */
    private static String className(TypeElement t) {
        return inLocal(t) ? elements.getBinaryName(t).toString() : t.getQualifiedName().toString();
    }

    /** Whether a type names a local or anonymous class anywhere in it. */
    private static boolean mentionsLocal(TypeMirror t) {
        if (t instanceof ArrayType a) {
            return mentionsLocal(a.getComponentType());
        } else if (t instanceof DeclaredType d) {
            return inLocal(d.asElement()) || d.getTypeArguments().stream().anyMatch(JavaTree::mentionsLocal);
        } else if (t instanceof ExecutableType x) {
            return mentionsLocal(x.getReturnType()) || x.getParameterTypes().stream().anyMatch(JavaTree::mentionsLocal);
        } else if (t instanceof WildcardType w) {
            return w.getExtendsBound() != null && mentionsLocal(w.getExtendsBound())
                    || w.getSuperBound() != null && mentionsLocal(w.getSuperBound());
        }
        return false;
    }

    /**
     * A type as javac prints it, without its type annotations, with local and anonymous classes
     * named by {@link #className}, and a class that javac could not find (with {@code --keep-going},
     * see {@link MissingClasses}) as {@code java.lang.Object}: only {@code null} is of such a type.
     */
    private static String typeText(TypeMirror t) {
        // Type annotations (`java.lang.@Nullable String`) say nothing the translation uses. javac
        // strips a generic method's type of its type parameters too, so those are kept.
        if (t instanceof com.sun.tools.javac.code.Type.ForAll generic) {
            t = new com.sun.tools.javac.code.Type.ForAll(generic.tvars, generic.qtype.stripMetadata());
        } else if (t instanceof com.sun.tools.javac.code.Type annotated) {
            t = annotated.stripMetadata();
        }
        if (t instanceof IntersectionType intersection) {
            return intersectionText(intersection);
        }
        boolean erroneous = keepGoing && ((com.sun.tools.javac.code.Type) t).isErroneous();
        if (!erroneous && !mentionsLocal(t)) {
            return t.toString();
        } else if (t.getKind() == TypeKind.ERROR) {
            return "java.lang.Object";
        } else if (t instanceof ArrayType a) {
            return typeText(a.getComponentType()) + "[]";
        } else if (t instanceof DeclaredType d) {
            String name = className((TypeElement) d.asElement());
            List<String> args = d.getTypeArguments().stream().map(JavaTree::typeText).toList();
            return args.isEmpty() ? name : name + "<" + String.join(",", args) + ">";
        } else if (t instanceof ExecutableType x) {
            List<String> params = x.getParameterTypes().stream().map(JavaTree::typeText).toList();
            return "(" + String.join(",", params) + ")" + typeText(x.getReturnType());
        } else if (t instanceof WildcardType w) {
            return w.getExtendsBound() != null ? "? extends " + typeText(w.getExtendsBound())
                    : "? super " + typeText(w.getSuperBound());
        }
        return t.toString();
    }

    /**
     * An intersection type, which javac prints as an empty name: its erasure, which is the type
     * the JVM gives its values (its first bound's, or {@code Object} for a type variable's), then
     * its other bounds but {@code Object}, joined by {@code " & "}: {@code Main.Base & Main.Named}.
     */
    private static String intersectionText(IntersectionType intersection) {
        TypeMirror erasure = types.erasure(intersection);
        StringBuilder text = new StringBuilder(typeText(erasure));
        for (TypeMirror bound : intersection.getBounds()) {
            String name = typeText(bound);
            if (!name.equals("java.lang.Object") && !types.isSameType(types.erasure(bound), erasure)) {
                text.append(" & ").append(name);
            }
        }
        return text.toString();
    }

    private static void string(String s, StringBuilder out) {
        if (unpaired(s)) {
            // JSON strings cannot hold an unpaired surrogate, so such a string is written as its
            // UTF-16 code units.
            out.append('[');
            for (int i = 0; i < s.length(); i++) {
                out.append(i == 0 ? "" : ",").append((int) s.charAt(i));
            }
            out.append(']');
            return;
        }
        out.append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
                case '"' -> out.append("\\\"");
                case '\\' -> out.append("\\\\");
                case '\n' -> out.append("\\n");
                case '\r' -> out.append("\\r");
                case '\t' -> out.append("\\t");
                default -> {
                    if (c < 0x20) {
                        out.append(String.format("\\u%04x", (int) c));
                    } else {
                        out.append(c);
                    }
                }
            }
        }
        out.append('"');
    }

    /** Whether {@code s} has a surrogate that is not part of a pair: it is not well-formed UTF-16. */
    private static boolean unpaired(String s) {
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (Character.isHighSurrogate(c) && i + 1 < s.length() && Character.isLowSurrogate(s.charAt(i + 1))) {
                i++;
            } else if (Character.isSurrogate(c)) {
                return true;
            }
        }
        return false;
    }
}
