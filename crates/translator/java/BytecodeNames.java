import com.sun.source.util.TaskEvent;
import com.sun.source.util.TaskListener;
import com.sun.tools.javac.api.JavacTaskImpl;
import com.sun.tools.javac.code.Flags;
import com.sun.tools.javac.code.Kinds;
import com.sun.tools.javac.code.Symbol;
import com.sun.tools.javac.code.Symbol.ClassSymbol;
import com.sun.tools.javac.code.Symbol.MethodSymbol;
import com.sun.tools.javac.code.Symbol.VarSymbol;
import com.sun.tools.javac.code.Type;
import com.sun.tools.javac.code.Types;
import com.sun.tools.javac.jvm.Gen;
import com.sun.tools.javac.tree.JCTree;
import com.sun.tools.javac.tree.JCTree.*;
import com.sun.tools.javac.tree.TreeInfo;
import com.sun.tools.javac.tree.TreeScanner;
import java.io.*;
import java.lang.reflect.Method;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.*;
import javax.tools.*;

/**
 * The names that the JVM's messages of a {@code NullPointerException} give the program's code
 * ("Cannot invoke \"Main$Box.size()\" because \"&lt;local2&gt;\" is null"), which a class file
 * compiled without {@code -g} decides: compiles the program as {@code javac} does, code
 * generation included, and prints what javac's code generator gave its code.
 *
 * <p>Usage: {@code java BytecodeNames [--patch-module MODULE=ROOT]... FILE...}, or the source on
 * standard input for one file. The files under the root of a module that the JDK has too, such
 * as javac's {@code jdk.compiler}, compile as part of that module, as the JDK compiles them; with
 * several such roots, each module's files compile on their own, against the JDK's other modules.
 * Prints one line, {@code {"java":"26","entries":[[CLASS,LINE,COLUMN,KIND,NAME,TEXT],...]}}:
 * the Java version, then for each use in the source (by the outermost class it is in, and the
 * line and column where it starts) of
 * <ul>
 * <li>{@code local}: a local variable or parameter named {@code NAME}, {@code TEXT} being how the
 * JVM names it, {@code <parameterN>} (the Nth parameter of the method of the class file, which
 * for a lambda's body starts with the locals it captures) or {@code <localN>} (its slot), or in a
 * module of the JDK, which the JDK compiles with {@code -g}, its name; not javac's own variables,
 * whose names have a {@code $};
 * <li>{@code temp}: a variable that javac adds for a statement, holding the value of the
 * expression that starts there ({@code arr$}, the array of a for-each loop; {@code i$}, its
 * iterator, at the start of the collection; {@code s$}, the string of a switch), named as a
 * local;
 * <li>{@code call}: a call of the method {@code NAME}, {@code TEXT} being the method as the class
 * file refers to it, {@code Main$Box.size()};
 * <li>{@code static}: a static field, {@code Main$Box.shared}.
 * </ul>
 * A variable of a {@code finally} block, which the code generator copies to every exit of its
 * {@code try}, has two slots where the copies differ: U+0001 {@code <localN>} U+0002
 * {@code <localM>} U+0003, its slot in the copies that run once the {@code try} completed, then
 * its slot in the copy for an exception, which keeps the exception in a slot of its own first
 * ({@code handlerShift}). {@code TEXT} is {@code null} where the JVM's name is not certain: a
 * variable of a {@code finally} block whose copies for a completed {@code try} differ (a
 * {@code return} or a jump out of a block with locals of its own) or of a {@code finally} block in
 * another, a variable of an instance initializer, copied into every constructor, one that the
 * code generator gives no slot, and a parameter that the method assigns, read in a
 * {@code finally} block whose copies name it differently. Such a parameter is a local where the
 * JVM's walk of the bytecode saw it written ({@link ParameterWrites}).
 */
public class BytecodeNames {
    public static void main(String[] args) throws Exception {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
        StandardJavaFileManager standard = compiler.getStandardFileManager(diagnostics, Locale.ROOT, StandardCharsets.UTF_8);
        // The class files stay in memory, for the local variable tables of `finally` blocks.
        JavaFileManager discarding = new ForwardingJavaFileManager<>(standard) {
            @Override
            public JavaFileObject getJavaFileForOutput(Location location, String className, JavaFileObject.Kind kind,
                    FileObject sibling) {
                return new SimpleJavaFileObject(URI.create("discarded:///" + className.replace('.', '/') + kind.extension), kind) {
                    @Override
                    public OutputStream openOutputStream() {
                        return new ByteArrayOutputStream() {
                            @Override
                            public void close() {
                                CLASS_FILES.put(className, toByteArray());
                            }
                        };
                    }
                };
            }
        };
        // `-g` adds the tables of the locals, which do not change their slots.
        List<String> options = new ArrayList<>(List.of("-proc:none", "-Xlint:none", "-nowarn", "-g"));
        // The translation reads `java.base` from its sources, where the program sees all its
        // packages, as the JDK's internal `sun.util.logging`: here they are exported to it too.
        java.lang.module.ModuleFinder.ofSystem().find("java.base").ifPresent(base -> {
            Set<String> exported = new HashSet<>();
            base.descriptor().exports().stream().filter(e -> !e.isQualified()).forEach(e -> exported.add(e.source()));
            for (String p : new TreeSet<>(base.descriptor().packages())) {
                if (!exported.contains(p)) {
                    options.addAll(List.of("--add-exports", "java.base/" + p + "=ALL-UNNAMED"));
                }
            }
        });
        List<File> files = new ArrayList<>();
        // The roots of the JDK's own modules, such as javac's `jdk.compiler`, by module: their
        // packages compile only as part of that module.
        Map<String, File> patches = new LinkedHashMap<>();
        for (int i = 0; i < args.length; i++) {
            if (args[i].equals("--patch-module") && i + 1 < args.length) {
                String[] patch = args[++i].split("=", 2);
                if (patch.length == 2 && java.lang.module.ModuleFinder.ofSystem().find(patch[0]).isPresent()) {
                    patches.put(patch[0], new File(patch[1]));
                }
            } else {
                files.add(new File(args[i]));
            }
        }
        // javac patches one module at most when it compiles without a module source path ("too
        // many patched modules"; given more, it reads a file of one module both as given and as
        // a source of the other, and fails an assertion): the files of each module compile on
        // their own, against the JDK's compiled modules, and the others without a patch.
        Map<String, List<File>> compilations = new LinkedHashMap<>();
        if (patches.size() <= 1) {
            compilations.put(patches.keySet().stream().findFirst().orElse(""), files);
        } else {
            for (File f : files) {
                String module = "";
                for (Map.Entry<String, File> patch : patches.entrySet()) {
                    if (f.getAbsoluteFile().toPath().normalize().startsWith(patch.getValue().getAbsoluteFile().toPath().normalize())) {
                        module = patch.getKey();
                        break;
                    }
                }
                compilations.computeIfAbsent(module, m -> new ArrayList<>()).add(f);
            }
        }
        Map<List<Object>, String> entries = new LinkedHashMap<>();
        for (Map.Entry<String, List<File>> compilation : compilations.entrySet()) {
            List<String> with = new ArrayList<>(options);
            if (!compilation.getKey().isEmpty()) {
                with.addAll(List.of("--patch-module", compilation.getKey() + "=" + patches.get(compilation.getKey())));
            }
            entries.putAll(compile(compiler, discarding, diagnostics, standard, with, compilation.getValue()));
        }
        // Written entry by entry: as one string, the names of a program as large as the Minecraft
        // server and its libraries pass the 2 GB that a Java string may hold.
        Writer stdout = new BufferedWriter(new OutputStreamWriter(new FileOutputStream(FileDescriptor.out), StandardCharsets.UTF_8), 1 << 16);
        stdout.write("{\"java\":\"" + Runtime.version().feature() + "\",\"entries\":[");
        StringBuilder out = new StringBuilder();
        boolean first = true;
        for (Map.Entry<List<Object>, String> e : entries.entrySet()) {
            out.setLength(0);
            out.append(first ? "" : ",").append('[');
            first = false;
            List<Object> key = e.getKey();
            string((String) key.get(0), out);
            out.append(',').append(key.get(1)).append(',').append(key.get(2)).append(',');
            string((String) key.get(3), out);
            out.append(',');
            string((String) key.get(4), out);
            out.append(',');
            if (e.getValue() == null) {
                out.append("null");
            } else {
                string(e.getValue(), out);
            }
            out.append(']');
            stdout.append(out);
        }
        stdout.write("]}\n");
        stdout.flush();
    }

    /**
     * Compiles {@code files} (none: the source on standard input) with {@code options}, and
     * returns the entries of their code; exits at the first error.
     */
    private static Map<List<Object>, String> compile(JavaCompiler compiler, JavaFileManager discarding,
            DiagnosticCollector<JavaFileObject> diagnostics, StandardJavaFileManager standard, List<String> options,
            List<File> files) throws Exception {
        Iterable<? extends JavaFileObject> sources;
        if (!files.isEmpty()) {
            sources = standard.getJavaFileObjectsFromFiles(files);
        } else {
            String source = new String(System.in.readAllBytes(), StandardCharsets.UTF_8);
            sources = List.of(new SimpleJavaFileObject(URI.create("string:///Main.java"), JavaFileObject.Kind.SOURCE) {
                @Override
                public CharSequence getCharContent(boolean ignoreEncodingErrors) {
                    return source;
                }

                @Override
                public boolean isNameCompatible(String simpleName, Kind kind) {
                    return true;
                }
            });
        }
        JavacTaskImpl task = (JavacTaskImpl) compiler.getTask(null, discarding, diagnostics, options, null, sources);
        BytecodeNames names = new BytecodeNames(task);
        task.addTaskListener(names.listener());
        task.generate();
        for (Diagnostic<? extends JavaFileObject> d : diagnostics.getDiagnostics()) {
            if (d.getKind() == Diagnostic.Kind.ERROR) {
                System.err.println(d.getLineNumber() + ":" + d.getColumnNumber() + ": " + d.getMessage(Locale.ROOT));
                System.exit(1);
            }
        }
        CLASS_FILES.clear();
        return names.entries;
    }

    private static void string(String s, StringBuilder out) {
        out.append('"');
        for (char c : s.toCharArray()) {
            if (c == '"' || c == '\\') {
                out.append('\\').append(c);
            } else if (c < 0x20) {
                out.append(String.format("\\u%04x", (int) c));
            } else {
                out.append(c);
            }
        }
        out.append('"');
    }

    /** The class files that the code generator wrote, by binary name. */
    private static final Map<String, byte[]> CLASS_FILES = new HashMap<>();

    private final Types types;
    private final Gen gen;
    /** javac's {@code Gen.binaryQualifier}: the class that a class file names as a member's owner. */
    private final Method qualifier;
    /** The classes of the program, by symbol, as attributed (lowering changes them in place). */
    private final Map<Symbol, JCClassDecl> classes = new HashMap<>();
    /** Where each node of the attributed trees starts, as line and column. */
    private final Map<JCTree, long[]> starts = new IdentityHashMap<>();
    /**
     * The local of each {@code ++w} and {@code --w}, with where the update starts, before the
     * local: a boxed local is unboxed there, which the translation looks the local up by.
     */
    private final Map<JCTree, long[]> prefixed = new IdentityHashMap<>();
    /** The members of each class as lowered, which the code generator discards once done. */
    private final Map<JCClassDecl, com.sun.tools.javac.util.List<JCTree>> lowered = new HashMap<>();
    /** The entries: class, line, column, kind and name, to the text or null. */
    private final Map<List<Object>, String> entries = new LinkedHashMap<>();

    private BytecodeNames(JavacTaskImpl task) throws ReflectiveOperationException {
        types = Types.instance(task.getContext());
        gen = Gen.instance(task.getContext());
        qualifier = Gen.class.getDeclaredMethod("binaryQualifier", Symbol.class, Type.class);
        qualifier.setAccessible(true);
    }

    private TaskListener listener() {
        return new TaskListener() {
            @Override
            public void started(TaskEvent e) {
                if (e.getKind() != TaskEvent.Kind.GENERATE) {
                    return;
                }
                JCClassDecl c = classes.get((Symbol) e.getTypeElement());
                if (c == null) {
                    return;
                }
                lowered.put(c, c.defs);
                // Flow analysis numbers variables in the same field that the code generator gives
                // slots in: a variable left at -1 got none.
                new TreeScanner() {
                    @Override
                    public void visitVarDef(JCVariableDecl tree) {
                        if (tree.sym != null && tree.sym.owner.kind == Kinds.Kind.MTH) {
                            tree.sym.adr = -1;
                        }
                        super.visitVarDef(tree);
                    }

                    @Override
                    public void visitClassDef(JCClassDecl tree) {
                        if (tree == c) {
                            super.visitClassDef(tree);
                        }
                    }
                }.scan(c);
            }

            @Override
            public void finished(TaskEvent e) {
                if (e.getKind() == TaskEvent.Kind.ANALYZE) {
                    JCCompilationUnit unit = (JCCompilationUnit) e.getCompilationUnit();
                    new TreeScanner() {
                        @Override
                        public void scan(JCTree tree) {
                            if (tree != null) {
                                int start = TreeInfo.getStartPos(tree);
                                if (start >= 0) {
                                    starts.put(tree, position(unit, start));
                                }
                                if (tree instanceof JCUnary u
                                        && (u.getTag() == JCTree.Tag.PREINC || u.getTag() == JCTree.Tag.PREDEC)
                                        && TreeInfo.skipParens(u.arg) instanceof JCIdent id) {
                                    prefixed.put(id, position(unit, start));
                                }
                                if (tree instanceof JCClassDecl c) {
                                    classes.put(c.sym, c);
                                }
                            }
                            super.scan(tree);
                        }
                    }.scan(unit);
                } else if (e.getKind() == TaskEvent.Kind.GENERATE) {
                    JCClassDecl c = classes.get((Symbol) e.getTypeElement());
                    if (c != null) {
                        report(c, lowered.remove(c), (JCCompilationUnit) e.getCompilationUnit());
                    }
                }
            }
        };
    }

    private static long[] position(JCCompilationUnit unit, int at) {
        return new long[] {unit.getLineMap().getLineNumber(at), unit.getLineMap().getColumnNumber(at)};
    }

    /**
     * How many slots further the variables of {@code finalizer}, a {@code finally} block of member
     * {@code def} of class {@code c} declaring {@code declared}, are in the copy that the code
     * generator writes for an exception than in the copies it writes where the {@code try}
     * completes; {@code null} where the latter do not agree (a {@code return} or a jump out of a
     * block of the {@code try}, with more locals in use). The copy for an exception is the last
     * the code generator writes, and the one whose slots the variables keep. The local variable
     * table of the class file (compiled with {@code -g}) has each copy of a named variable of the
     * block, apart by where it starts.
     */
    private static Integer handlerShift(JCClassDecl c, JCTree def, JCBlock finalizer, List<VarSymbol> declared,
            JCCompilationUnit unit) {
        byte[] bytes = CLASS_FILES.get(c.sym.flatname.toString());
        VarSymbol named = declared.stream()
            .filter(v -> v.adr >= 0 && !v.name.toString().contains("$"))
            .findFirst()
            .orElse(null);
        if (bytes == null || named == null || !(def instanceof JCMethodDecl method)) {
            return null;
        }
        long from = position(unit, TreeInfo.getStartPos(finalizer))[0];
        long to = position(unit, TreeInfo.endPos(finalizer))[0];
        java.lang.classfile.ClassModel model = java.lang.classfile.ClassFile.of().parse(bytes);
        for (java.lang.classfile.MethodModel m : model.methods()) {
            if (!m.methodName().equalsString(method.sym.name.toString())) {
                continue;
            }
            java.lang.classfile.attribute.CodeAttribute code =
                m.findAttribute(java.lang.classfile.Attributes.code()).orElse(null);
            if (code == null) {
                continue;
            }
            List<java.lang.classfile.attribute.LineNumberInfo> lines = code
                .findAttribute(java.lang.classfile.Attributes.lineNumberTable())
                .map(java.lang.classfile.attribute.LineNumberTableAttribute::lineNumbers)
                .orElse(List.of());
            List<java.lang.classfile.attribute.LocalVariableInfo> slots = new ArrayList<>();
            for (java.lang.classfile.attribute.LocalVariableInfo v : code
                    .findAttribute(java.lang.classfile.Attributes.localVariableTable())
                    .map(java.lang.classfile.attribute.LocalVariableTableAttribute::localVariables)
                    .orElse(List.of())) {
                // The line of the last entry of the line table that starts before the variable.
                int line = -1;
                int at = -1;
                for (java.lang.classfile.attribute.LineNumberInfo l : lines) {
                    if (l.startPc() <= v.startPc() && l.startPc() >= at) {
                        at = l.startPc();
                        line = l.lineNumber();
                    }
                }
                if (v.name().equalsString(named.name.toString()) && line >= from && line <= to) {
                    slots.add(v);
                }
            }
            slots.sort(Comparator.comparingInt(java.lang.classfile.attribute.LocalVariableInfo::startPc));
            if (slots.size() < 2 || slots.get(slots.size() - 1).slot() != named.adr) {
                continue;
            }
            int normal = slots.get(0).slot();
            for (int i = 1; i < slots.size() - 1; i++) {
                if (slots.get(i).slot() != normal) {
                    return null;
                }
            }
            int shift = named.adr - normal;
            return shift >= 0 ? shift : null;
        }
        return null;
    }

    /** The entries of the members of class {@code c}, lowered to {@code defs} and generated. */
    private void report(JCClassDecl c, com.sun.tools.javac.util.List<JCTree> defs, JCCompilationUnit unit) {
        String outermost = c.sym.outermostClass().flatname.toString();
        // A class of one of the JDK's modules (`--patch-module`), such as javac's.
        boolean jdkModule = !c.sym.packge().modle.isUnnamed();
        for (JCTree def : defs) {
            List<VarSymbol> params = new ArrayList<>();
            // The variables whose slots are not certain, and the parameters that the method assigns.
            Set<Symbol> uncertain = new HashSet<>();
            // The slots of the variables of a `finally` block in the copies that run after the
            // `try` completes and in the one that runs for an exception (see `handlerShift`).
            Map<Symbol, int[]> copies = new HashMap<>();
            Set<Symbol> assigned = new HashSet<>();
            JCTree body;
            if (def instanceof JCMethodDecl m && m.body != null) {
                body = m.body;
                for (JCVariableDecl p : m.params) {
                    params.add(p.sym);
                }
            } else if (def instanceof JCBlock || def instanceof JCVariableDecl) {
                body = def;
                // An instance initializer is copied into every constructor.
                if ((TreeInfo.flags(def) & Flags.STATIC) == 0) {
                    new TreeScanner() {
                        @Override
                        public void visitVarDef(JCVariableDecl v) {
                            uncertain.add(v.sym);
                            super.visitVarDef(v);
                        }
                    }.scan(def);
                }
            } else {
                continue;
            }
            new TreeScanner() {
                private void assigned(JCTree target) {
                    if (TreeInfo.skipParens((JCExpression) target) instanceof JCIdent id && params.contains(id.sym)) {
                        assigned.add(id.sym);
                    }
                }

                @Override
                public void visitAssign(JCAssign t) {
                    assigned(t.lhs);
                    super.visitAssign(t);
                }

                @Override
                public void visitAssignop(JCAssignOp t) {
                    assigned(t.lhs);
                    super.visitAssignop(t);
                }

                @Override
                public void visitUnary(JCUnary t) {
                    if (t.getTag().isIncOrDecUnaryOp()) {
                        assigned(t.arg);
                    }
                    super.visitUnary(t);
                }

                /** How many `finally` blocks the scan is in. */
                private int finallies;

                @Override
                public void visitTry(JCTry t) {
                    scan(t.resources);
                    scan(t.body);
                    scan(t.catchers);
                    if (t.finalizer != null) {
                        // Its variables, but those of a `finally` in it, which is copied more.
                        List<VarSymbol> declared = new ArrayList<>();
                        new TreeScanner() {
                            private boolean nested;

                            @Override
                            public void visitVarDef(JCVariableDecl v) {
                                uncertain.add(v.sym);
                                if (!nested) {
                                    declared.add(v.sym);
                                }
                                super.visitVarDef(v);
                            }

                            @Override
                            public void visitTry(JCTry inner) {
                                scan(inner.resources);
                                scan(inner.body);
                                scan(inner.catchers);
                                boolean outer = nested;
                                nested = true;
                                scan(inner.finalizer);
                                nested = outer;
                            }
                        }.scan(t.finalizer);
                        // The copies of a `finally` in another one are not told apart.
                        if (finallies == 0) {
                            Integer shift = handlerShift(c, def, t.finalizer, declared, unit);
                            for (VarSymbol v : declared) {
                                if (shift != null && v.adr >= shift) {
                                    copies.put(v, new int[] {v.adr - shift, v.adr});
                                }
                            }
                        }
                        finallies++;
                        scan(t.finalizer);
                        finallies--;
                    }
                }
            }.scan(body);
            ParameterWrites writes = new ParameterWrites(assigned);
            if (!assigned.isEmpty()) {
                writes.scan(body);
            }
            new TreeScanner() {
                /** Where the source has the variables that lowering added: where their values start. */
                private final Map<Symbol, JCTree> temps = new HashMap<>();

                private void entry(JCTree at, String kind, String name, String text) {
                    if (!starts.containsKey(at) && at instanceof JCIdent id && temps.containsKey(id.sym)) {
                        at = temps.get(id.sym);
                    }
                    long[] start = starts.get(at);
                    if (start == null) {
                        // A name that lowering wrote anew, such as a lambda's captured local in
                        // its body, or a static that it qualifies with its class (an enclosing
                        // class's, or one imported), is where the name it stands for was.
                        if (!(at instanceof JCIdent || at instanceof JCFieldAccess && kind.equals("static"))) {
                            return;
                        }
                        start = position(unit, at.pos);
                    }
                    put(start, kind, name, text);
                }

                private void put(long[] start, String kind, String name, String text) {
                    List<Object> key = List.of(outermost, start[0], start[1], kind, name);
                    if (entries.containsKey(key) && !Objects.equals(entries.get(key), text)) {
                        text = null;
                    }
                    entries.put(key, text);
                }

                /** {@code e} as the source has it: without the casts that erasure added. */
                private JCTree written(JCTree e) {
                    while (!starts.containsKey(e) && (e instanceof JCTypeCast || e instanceof JCParens)) {
                        e = e instanceof JCTypeCast cast ? cast.expr : ((JCParens) e).expr;
                    }
                    return e;
                }

                /** How the JVM names variable {@code v} where {@code use} reads it. */
                private String local(VarSymbol v, JCIdent use) {
                    // A variable of a `finally` block: the slot of its copies that run once the
                    // `try` completed and that of the copy for an exception, as
                    // U+0001 `<localN>` U+0002 `<localM>` U+0003.
                    int[] slots = copies.get(v);
                    if (slots != null) {
                        return slots[0] == slots[1]
                            ? "<local" + slots[0] + ">"
                            : "\u0001<local" + slots[0] + ">\u0002<local" + slots[1] + ">\u0003";
                    }
                    if (v.adr < 0 || uncertain.contains(v)) {
                        return null;
                    }
                    int index = params.indexOf(v);
                    if (index >= 0 && assigned.contains(v)) {
                        // Each copy of the code that reads it (a `finally` block's) must agree.
                        Set<Boolean> written = writes.uses.get(use);
                        if (written == null || written.size() != 1) {
                            return null;
                        }
                        if (written.contains(true) || v.adr >= ParameterWrites.SLOTS) {
                            return "<local" + v.adr + ">";
                        }
                    }
                    return index >= 0 ? "<parameter" + (index + 1) + ">" : "<local" + v.adr + ">";
                }

                @Override
                public void visitIdent(JCIdent t) {
                    // The names with a `$` are javac's own (`arr$`), the temps.
                    if (t.sym instanceof VarSymbol v && v.owner.kind == Kinds.Kind.MTH) {
                        // The JDK compiles its modules with the names of their locals (`-g`).
                        if (!t.name.toString().contains("$")) {
                            String text = jdkModule ? t.name.toString() : local(v, t);
                            entry(t, "local", t.name.toString(), text);
                            if (prefixed.containsKey(t)) {
                                put(prefixed.get(t), "local", t.name.toString(), text);
                            }
                        }
                    } else if (t.sym instanceof VarSymbol v && v.isStatic() && v.getConstValue() == null) {
                        entry(t, "static", t.name.toString(), member(v, c.sym.type));
                    }
                    super.visitIdent(t);
                }

                @Override
                public void visitSelect(JCFieldAccess t) {
                    if (t.sym instanceof VarSymbol v && v.isStatic() && v.getConstValue() == null) {
                        entry(t, "static", t.name.toString(), member(v, t.selected.type));
                    }
                    super.visitSelect(t);
                }

                @Override
                public void visitVarDef(JCVariableDecl t) {
                    // A variable that lowering added, which the attributed tree does not have
                    // (javac names its own with a `$`; a for-each loop's variable is declared
                    // anew too, but it is the source's).
                    if (t.sym != null && !starts.containsKey(t) && t.init != null && t.name.toString().contains("$")) {
                        // A call that lowering wrote, such as the `iterator()` of a for-each loop
                        // (`i$`), starts where its receiver does.
                        JCTree init = TreeInfo.skipParens(t.init);
                        if (!starts.containsKey(init) && init instanceof JCMethodInvocation call
                                && call.meth instanceof JCFieldAccess f) {
                            init = written(f.selected);
                        }
                        temps.put(t.sym, init);
                        entry(init, "temp", t.name.toString().replaceAll("[0-9]", ""), local(t.sym, null));
                    }
                    super.visitVarDef(t);
                }

                @Override
                public void visitApply(JCMethodInvocation t) {
                    if (TreeInfo.symbol(t.meth) instanceof MethodSymbol m && !m.isDynamic()) {
                        Type site = t.meth instanceof JCFieldAccess f ? f.selected.type : c.sym.type;
                        // A call that lowering wrote, such as the `iterator()` of a for-each
                        // loop, starts where its receiver does.
                        JCTree at = starts.containsKey(t) || !(t.meth instanceof JCFieldAccess f) ? t : written(f.selected);
                        entry(at, "call", m.name.toString(), member(m, site));
                        // Calls of one name that start alike, as in a chain
                        // (`StreamSupport.stream(s, false).map(f).collect(c).stream()`), apart
                        // by where their first argument starts: `stream@3:30`, `stream@`. Calls
                        // without one are apart by the class of the receiver too:
                        // `stream@#java.util.List`.
                        long[] first = t.args.isEmpty() ? null : starts.get(written(t.args.head));
                        if (t.args.isEmpty() || first != null) {
                            String where = first == null ? "" : first[0] + ":" + first[1];
                            entry(at, "call", m.name + "@" + where, member(m, site));
                        }
                        if (t.args.isEmpty() && types.erasure(site).tsym instanceof ClassSymbol receiver) {
                            entry(at, "call", m.name + "@#" + receiver.flatname, member(m, site));
                        }
                    }
                    super.visitApply(t);
                }
            }.scan(body);
        }
    }

    /**
     * Whether the JVM names a parameter that its method assigns as the parameter
     * ({@code <parameterN>}) or as a local ({@code <localN>}) where the code reads it. HotSpot's
     * message ({@code ExceptionMessageBuilder}) names it as a local where its slot was written on
     * a path that runs forward through the bytecode, from the method's start or from the start
     * of an exception handler (where no slot is written), to the load: it walks the bytecode once
     * in order, merging what it has seen into each jump's target, and stops at the failing
     * instruction, so a write that only a jump back brings (a loop's next round) does not count.
     * This walks the lowered code in the order of javac's code generator ({@code Gen}), with its
     * jumps: a loop's condition before its body and its step, so the loop is left from the
     * condition as it was first reached; a {@code finally} block copied to each way out of its
     * {@code try} and to the handler of any exception.
     */
    private static final class ParameterWrites extends TreeScanner {
        /** HotSpot keeps the writes of the first 64 slots; a later slot counts as written. */
        static final int SLOTS = 64;

        private final Set<Symbol> params;
        /** For each read of a parameter, whether its slot was written: both, where copies differ. */
        final Map<JCIdent, Set<Boolean>> uses = new IdentityHashMap<>();
        /** The parameters written on the paths that reach here; {@code null} where none does. */
        private Set<Symbol> state = Set.of();
        /** What the jumps to each statement's end ({@code break}, {@code yield}) bring. */
        private final Map<JCTree, Set<Symbol>> exits = new IdentityHashMap<>();
        /** What the {@code continue}s of each loop bring. */
        private final Map<JCTree, Set<Symbol>> continues = new IdentityHashMap<>();
        /** The {@code try}s with a {@code finally} around the code, innermost last. */
        private final List<JCTry> finalizers = new ArrayList<>();
        /** How many of {@code finalizers} are around each statement that a jump may leave. */
        private final Map<JCTree, Integer> depth = new IdentityHashMap<>();

        ParameterWrites(Set<Symbol> params) {
            this.params = params;
        }

        private static Set<Symbol> union(Set<Symbol> a, Set<Symbol> b) {
            if (a == null) {
                return b;
            }
            if (b == null || a.containsAll(b)) {
                return a;
            }
            Set<Symbol> both = new HashSet<>(a);
            both.addAll(b);
            return both;
        }

        private void write(Symbol param) {
            if (state != null && !state.contains(param)) {
                Set<Symbol> next = new HashSet<>(state);
                next.add(param);
                state = next;
            }
        }

        /** Whether {@code cond} is a constant, which javac's code generator jumps on or not at all. */
        private static boolean isTrue(JCExpression cond) {
            return cond == null || cond.type != null && cond.type.isTrue();
        }

        private static boolean isFalse(JCExpression cond) {
            return cond != null && cond.type != null && cond.type.isFalse();
        }

        @Override
        public void scan(JCTree tree) {
            // Code that no path reaches is not generated.
            if (tree != null && state != null) {
                super.scan(tree);
            }
        }

        @Override
        public void visitClassDef(JCClassDecl t) {
            // Another class's code.
        }

        @Override
        public void visitLambda(JCLambda t) {
            // Another method's code.
        }

        @Override
        public void visitIdent(JCIdent t) {
            if (state != null && params.contains(t.sym)) {
                uses.computeIfAbsent(t, k -> new HashSet<>()).add(state.contains(t.sym));
            }
        }

        private JCIdent param(JCExpression target) {
            return TreeInfo.skipParens(target) instanceof JCIdent id && params.contains(id.sym) ? id : null;
        }

        @Override
        public void visitAssign(JCAssign t) {
            JCIdent target = param(t.lhs);
            if (target == null) {
                super.visitAssign(t);
            } else {
                // The value is computed before it is stored.
                scan(t.rhs);
                write(target.sym);
            }
        }

        @Override
        public void visitAssignop(JCAssignOp t) {
            JCIdent target = param(t.lhs);
            super.visitAssignop(t);
            if (target != null) {
                write(target.sym);
            }
        }

        @Override
        public void visitUnary(JCUnary t) {
            JCIdent target = t.getTag().isIncOrDecUnaryOp() ? param(t.arg) : null;
            super.visitUnary(t);
            if (target != null) {
                write(target.sym);
            }
        }

        @Override
        public void visitIf(JCIf t) {
            scan(t.cond);
            Set<Symbol> start = state, then = null, otherwise = null;
            if (!isFalse(t.cond)) {
                scan(t.thenpart);
                then = state;
            }
            if (!isTrue(t.cond)) {
                state = start;
                scan(t.elsepart);
                otherwise = state;
            }
            state = union(then, otherwise);
        }

        @Override
        public void visitConditional(JCConditional t) {
            scan(t.cond);
            Set<Symbol> start = state, then = null, otherwise = null;
            if (!isFalse(t.cond)) {
                scan(t.truepart);
                then = state;
            }
            if (!isTrue(t.cond)) {
                state = start;
                scan(t.falsepart);
                otherwise = state;
            }
            state = union(then, otherwise);
        }

        @Override
        public void visitWhileLoop(JCWhileLoop t) {
            loop(t, t.cond, t.body, com.sun.tools.javac.util.List.nil());
        }

        @Override
        public void visitForLoop(JCForLoop t) {
            scan(t.init);
            loop(t, t.cond, t.body, t.step);
        }

        /** A loop whose condition comes first. */
        private void loop(JCStatement loop, JCExpression cond, JCStatement body, com.sun.tools.javac.util.List<? extends JCTree> step) {
            depth.put(loop, finalizers.size());
            scan(cond);
            Set<Symbol> exit = isTrue(cond) ? null : state;
            if (!isFalse(cond)) {
                scan(body);
                // `continue` goes to the step, which goes back to the condition.
                state = union(state, continues.remove(loop));
                scan(step);
            }
            state = union(exit, exits.remove(loop));
        }

        @Override
        public void visitDoLoop(JCDoWhileLoop t) {
            depth.put(t, finalizers.size());
            scan(t.body);
            state = union(state, continues.remove(t));
            Set<Symbol> exit = null;
            if (state != null) {
                scan(t.cond);
                exit = isTrue(t.cond) ? null : state;
            }
            state = union(exit, exits.remove(t));
        }

        @Override
        public void visitLabelled(JCLabeledStatement t) {
            depth.put(t, finalizers.size());
            scan(t.body);
            state = union(state, exits.remove(t));
        }

        @Override
        public void visitSwitch(JCSwitch t) {
            switchCases(t, t.selector, t.cases);
        }

        @Override
        public void visitSwitchExpression(JCSwitchExpression t) {
            switchCases(t, t.selector, t.cases);
        }

        /** The cases follow the switch's jump to each, or the case before them. */
        private void switchCases(JCTree t, JCExpression selector, com.sun.tools.javac.util.List<JCCase> cases) {
            scan(selector);
            depth.put(t, finalizers.size());
            Set<Symbol> start = state, previous = null;
            boolean otherwise = false;
            for (JCCase c : cases) {
                otherwise |= c.labels.stream().anyMatch(l -> l instanceof JCDefaultCaseLabel);
                state = union(start, previous);
                scan(c.stats);
                previous = state;
            }
            state = union(previous, exits.remove(t));
            if (!otherwise) {
                state = union(state, start);
            }
        }

        @Override
        public void visitBreak(JCBreak t) {
            jump(t.target, exits);
        }

        @Override
        public void visitYield(JCYield t) {
            scan(t.value);
            jump(t.target, exits);
        }

        @Override
        public void visitContinue(JCContinue t) {
            jump(t.target, continues);
        }

        @Override
        public void visitReturn(JCReturn t) {
            scan(t.expr);
            finalizers(0);
            state = null;
        }

        @Override
        public void visitThrow(JCThrow t) {
            scan(t.expr);
            state = null;
        }

        private void jump(JCTree target, Map<JCTree, Set<Symbol>> to) {
            finalizers(depth.getOrDefault(target, 0));
            if (state != null) {
                to.merge(target, state, ParameterWrites::union);
            }
            state = null;
        }

        /** The copies of the {@code finally} blocks that a jump out of them runs, innermost first. */
        private void finalizers(int outermost) {
            List<JCTry> around = new ArrayList<>(finalizers);
            for (int i = around.size() - 1; i >= outermost; i--) {
                // A block runs outside its own `try`.
                finalizers.subList(i, finalizers.size()).clear();
                scan(around.get(i).finalizer);
            }
            finalizers.clear();
            finalizers.addAll(around);
        }

        @Override
        public void visitTry(JCTry t) {
            scan(t.resources);
            boolean fin = t.finalizer != null;
            Set<Symbol> exit = null;
            List<JCTree> blocks = new ArrayList<>();
            blocks.add(t.body);
            for (JCCatch c : t.catchers) {
                blocks.add(c.body);
            }
            for (int i = 0; i < blocks.size(); i++) {
                if (i > 0) {
                    // An exception handler starts with no slot written.
                    state = Set.of();
                }
                if (fin) {
                    finalizers.add(t);
                }
                scan(blocks.get(i));
                if (fin) {
                    finalizers.remove(finalizers.size() - 1);
                    scan(t.finalizer);
                }
                exit = union(exit, state);
            }
            if (fin) {
                // The handler of any other exception, which runs the block and throws it again.
                state = Set.of();
                scan(t.finalizer);
            }
            state = exit;
        }
    }

    /** A field or method as a class file refers to it, and the JVM's messages show it. */
    private String member(Symbol s, Type site) {
        Symbol q;
        try {
            q = (Symbol) qualifier.invoke(gen, s, site);
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException(e);
        }
        String owner = className(types.erasure(q.owner.type));
        if (q instanceof VarSymbol) {
            return owner + "." + q.name;
        }
        StringBuilder out = new StringBuilder(owner).append('.').append(q.name).append('(');
        String separator = "";
        for (Type p : q.externalType(types).getParameterTypes()) {
            out.append(separator).append(className(types.erasure(p)));
            separator = ", ";
        }
        return out.append(')').toString();
    }

    /** A type as the JVM's messages show it: its binary name, without {@code java.lang.} for
     * {@code Object} and {@code String}. */
    private static String className(Type t) {
        if (t instanceof Type.ArrayType a) {
            return className(a.elemtype) + "[]";
        }
        if (t.isPrimitive() || !(t.tsym instanceof ClassSymbol c)) {
            return t.toString();
        }
        String name = c.flatname.toString();
        return switch (name) {
            case "java.lang.Object" -> "Object";
            case "java.lang.String" -> "String";
            default -> name;
        };
    }
}
