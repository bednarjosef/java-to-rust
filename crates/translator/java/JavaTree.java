import com.sun.source.tree.*;
import com.sun.source.util.*;
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
     * Whole-program mode: {@code java JavaTree --program [--sourcepath PATH] FILE...} compiles all
     * the files in one javac run, loading the other classes they use from the source path, and
     * prints {@code {"units":[{"file":...,"tree":...},...]}}: the files given, in order, then every
     * unit javac loaded from the source path and attributed. Compile errors are printed as
     * {@code FILE:LINE:COLUMN: message}.
     */
    private static void program(List<String> args) throws Exception {
        List<String> options = new ArrayList<>(List.of("-proc:none", "-Xlint:none", "-XDshould-stop.at=FLOW"));
        List<File> files = new ArrayList<>();
        for (int i = 0; i < args.size(); i++) {
            if (args.get(i).equals("--sourcepath") && i + 1 < args.size()) {
                options.add("-sourcepath");
                options.add(args.get(++i));
            } else {
                files.add(new File(args.get(i)));
            }
        }
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
        StandardJavaFileManager fileManager = compiler.getStandardFileManager(diagnostics, Locale.ROOT, StandardCharsets.UTF_8);
        JavacTask task = (JavacTask) compiler.getTask(null, fileManager, diagnostics, options, null,
                fileManager.getJavaFileObjectsFromFiles(files));
        // Units that javac loads from the source path are attributed too, but only reported here.
        Set<CompilationUnitTree> analyzed = new LinkedHashSet<>();
        task.addTaskListener(new TaskListener() {
            @Override
            public void finished(TaskEvent e) {
                if (e.getKind() == TaskEvent.Kind.ANALYZE) {
                    analyzed.add(e.getCompilationUnit());
                }
            }
        });
        List<CompilationUnitTree> units = new ArrayList<>();
        elements = task.getElements();
        types = task.getTypes();
        task.parse().forEach(units::add);
        task.analyze();
        boolean failed = false;
        for (Diagnostic<? extends JavaFileObject> d : diagnostics.getDiagnostics()) {
            if (d.getKind() == Diagnostic.Kind.ERROR) {
                String file = d.getSource() == null ? "" : d.getSource().getName();
                System.err.println(file + ":" + d.getLineNumber() + ":" + d.getColumnNumber() + ": " + d.getMessage(Locale.ROOT));
                failed = true;
            }
        }
        if (failed) {
            System.exit(1);
        }
        for (CompilationUnitTree unit : analyzed) {
            if (!units.contains(unit)) {
                units.add(unit);
            }
        }
        Trees trees = Trees.instance(task);
        StringBuilder out = new StringBuilder("{\"units\":[");
        for (CompilationUnitTree unit : units) {
            if (unit != units.get(0)) {
                out.append(',');
            }
            out.append("{\"file\":");
            string(unit.getSourceFile().getName(), out);
            out.append(",\"tree\":");
            new JavaTree(trees, unit).node(new TreePath(unit), out);
            out.append('}');
        }
        out.append("]}\n");
        PrintStream stdout = new PrintStream(new FileOutputStream(FileDescriptor.out), false, StandardCharsets.UTF_8);
        stdout.print(out);
        stdout.flush();
    }

    private static Elements elements;
    private static Types types;

    private final Trees trees;
    private final CompilationUnitTree unit;
    private final SourcePositions positions;

    private JavaTree(Trees trees, CompilationUnitTree unit) {
        this.trees = trees;
        this.unit = unit;
        this.positions = trees.getSourcePositions();
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
        if (tree instanceof ExpressionTree) {
            TypeMirror type = trees.getTypeMirror(path);
            if (type != null) {
                out.append(",\"javaType\":");
                string(typeText(type), out);
            }
        }
        if (tree instanceof IdentifierTree || tree instanceof MemberSelectTree || tree instanceof MethodInvocationTree
                || tree instanceof NewClassTree || tree instanceof VariableTree || tree instanceof MethodTree
                || tree instanceof ClassTree || tree instanceof MemberReferenceTree) {
            Element element = trees.getElement(path);
            if (element != null) {
                out.append(",\"symbol\":");
                symbol(element, tree instanceof MethodTree, out);
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
            string(owner instanceof TypeElement t ? className(t) : owner.toString(), out);
        }
        out.append(",\"static\":").append(e.getModifiers().contains(Modifier.STATIC));
        // javac prints a varargs parameter's type as an array, so only this says it is varargs.
        if (e instanceof ExecutableElement m && m.isVarArgs()) {
            out.append(",\"varargs\":true");
        }
        out.append(",\"type\":");
        string(typeText(e.asType()), out);
        if (declaration && e.getKind() == ElementKind.METHOD && owner instanceof TypeElement t) {
            overrides((ExecutableElement) e, t, out);
        }
        // The value of a constant variable, as text: javac's compile-time constant.
        if (e instanceof VariableElement v && v.getConstantValue() != null) {
            Object c = v.getConstantValue();
            out.append(",\"constant\":");
            string(c instanceof Character ch ? Integer.toString(ch) : c.toString(), out);
        }
        out.append('}');
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

    /** A type as javac prints it, with local and anonymous classes named by {@link #className}. */
    private static String typeText(TypeMirror t) {
        if (!mentionsLocal(t)) {
            return t.toString();
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
