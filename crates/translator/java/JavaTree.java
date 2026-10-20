import com.sun.source.tree.*;
import com.sun.source.util.*;
import java.io.*;
import java.lang.reflect.Method;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.*;
import javax.lang.model.element.*;
import javax.lang.model.type.TypeMirror;
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
 * {@code javaType}; names, calls, {@code new}, and declarations get {@code symbol}.
 */
public class JavaTree {
    public static void main(String[] args) throws Exception {
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
                string(type.toString(), out);
            }
        }
        if (tree instanceof IdentifierTree || tree instanceof MemberSelectTree || tree instanceof MethodInvocationTree
                || tree instanceof NewClassTree || tree instanceof VariableTree || tree instanceof MethodTree
                || tree instanceof ClassTree || tree instanceof MemberReferenceTree) {
            Element element = trees.getElement(path);
            if (element != null) {
                out.append(",\"symbol\":");
                symbol(element, out);
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

    private void symbol(Element e, StringBuilder out) {
        out.append("{\"kind\":");
        string(e.getKind().name(), out);
        out.append(",\"name\":");
        string(e.getSimpleName().toString(), out);
        Element owner = e.getEnclosingElement();
        if (owner != null) {
            out.append(",\"owner\":");
            string(owner instanceof TypeElement t ? t.getQualifiedName().toString() : owner.toString(), out);
        }
        out.append(",\"static\":").append(e.getModifiers().contains(Modifier.STATIC));
        out.append(",\"type\":");
        string(e.asType().toString(), out);
        out.append('}');
    }

    private static void string(String s, StringBuilder out) {
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
                    if (c < 0x20 || Character.isSurrogate(c)) {
                        out.append(String.format("\\u%04x", (int) c));
                    } else {
                        out.append(c);
                    }
                }
            }
        }
        out.append('"');
    }
}
