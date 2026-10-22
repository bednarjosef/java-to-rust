import com.sun.source.tree.*;
import com.sun.source.util.*;
import java.io.*;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.regex.*;
import javax.tools.*;

/**
 * The translator's patches of library classes, in {@code java/patches} (see docs/jdk-overlays.md).
 * A patch is a Java file at the path of the class it patches ({@code java/io/PrintStream.java}),
 * which holds only what differs from the library's file. Each of its members says what it does
 * with a marker annotation:
 *
 * <ul>
 *   <li>{@code @Replace}: the library's member of the same name and parameter types becomes this
 *       one.
 *   <li>{@code @Add}: this member is added; the library's class must not have one of that name
 *       and parameter types.
 *   <li>{@code @Remove}: the library's member of that name and parameter types is left out (the
 *       patch declares it without a body).
 *   <li>{@code @Edit(find = "...", replace = "...")}: inside the library's member (declared
 *       without a body), the text {@code find}, which must be there exactly once, becomes
 *       {@code replace}; with {@code from} and {@code to} in place of {@code find}, the text from
 *       {@code from} (there exactly once) through the first {@code to} after it. On the class
 *       itself, it edits the whole class. Runs of whitespace in the texts match any run of
 *       whitespace.
 *   <li>{@code @Patch}: on a nested class, patches its members the same way.
 * </ul>
 *
 * <p>An initializer block of the patch replaces the class's only one of its kind (static or
 * not); an empty one removes it. The patch's imports are added to the file's. Everything else of the library's file is kept
 * as it is. A patch that names a member the library's class does not have, or that adds one it
 * has, is refused, so a change of the library's code cannot skip a patch silently.
 *
 * <p>{@link #patching} gives javac the patched files in place of the library's;
 * {@code java JdkPatch PATCHES ROOT... [--out DIR]} applies every patch to the file of the same
 * path in the first root that has it, and prints why any is refused (with {@code --out}, the
 * patched files are written under DIR).
 */
final class JdkPatch {
    /** Why a patch cannot be applied. */
    static final class Refused extends Exception {
        Refused(String message) {
            super(message);
        }
    }

    /** The names of the marker annotations. */
    private static final Set<String> MARKERS = Set.of("Replace", "Add", "Remove", "Edit", "Patch");

    /** The environment variable that names a directory to write every patched file into. */
    private static final String SHOW = "JAVA_TO_RUST_PATCHED_SOURCES";

    public static void main(String[] args) throws IOException {
        Path out = null;
        List<Path> paths = new ArrayList<>();
        for (int i = 0; i < args.length; i++) {
            if (args[i].equals("--out") && i + 1 < args.length) {
                out = Path.of(args[++i]);
            } else {
                paths.add(Path.of(args[i]));
            }
        }
        if (paths.size() < 2) {
            System.err.println("usage: java JdkPatch PATCHES ROOT... [--out DIR]");
            System.exit(2);
        }
        Path patches = paths.get(0);
        List<Path> roots = paths.subList(1, paths.size());
        List<Path> files;
        try (var walk = Files.walk(patches)) {
            files = walk.filter(p -> p.toString().endsWith(".java")).sorted().toList();
        }
        boolean failed = false;
        for (Path patch : files) {
            Path relative = patches.relativize(patch);
            Path original = roots.stream().map(r -> r.resolve(relative)).filter(Files::isRegularFile).findFirst()
                    .orElse(null);
            if (original == null) {
                System.err.println(patch + ": no library has " + relative + " to patch");
                failed = true;
                continue;
            }
            try {
                String patched = apply(patch.toString(), Files.readString(patch), relative.toString(),
                        Files.readString(original));
                if (out != null) {
                    Files.createDirectories(out.resolve(relative).getParent());
                    Files.writeString(out.resolve(relative), patched);
                }
            } catch (Refused e) {
                System.err.println(e.getMessage());
                failed = true;
            }
        }
        System.exit(failed ? 1 : 0);
    }

    /**
     * The file manager that gives javac, for each source file of the source path that a patch
     * under {@code patches} names, the file as patched. Without {@code patches} it is
     * {@code fileManager}.
     */
    static JavaFileManager patching(JavaFileManager fileManager, Path patches) {
        if (patches == null) {
            return fileManager;
        }
        return new ForwardingJavaFileManager<>(fileManager) {
            private final Map<URI, Patched> made = new HashMap<>();

            private JavaFileObject wrap(Location location, JavaFileObject file) {
                if (location != StandardLocation.SOURCE_PATH || file == null || file.getKind() != JavaFileObject.Kind.SOURCE) {
                    return file;
                }
                String binary = super.inferBinaryName(location, file);
                if (binary == null || binary.endsWith("module-info") || binary.endsWith("package-info")) {
                    return file;
                }
                Path patch = patches.resolve(binary.replace('.', '/') + ".java");
                if (!Files.isRegularFile(patch)) {
                    return file;
                }
                return made.computeIfAbsent(file.toUri(), u -> new Patched(file, patch, binary));
            }

            @Override
            public Iterable<JavaFileObject> list(Location location, String packageName, Set<JavaFileObject.Kind> kinds,
                    boolean recurse) throws IOException {
                List<JavaFileObject> out = new ArrayList<>();
                for (JavaFileObject file : super.list(location, packageName, kinds, recurse)) {
                    out.add(wrap(location, file));
                }
                return out;
            }

            @Override
            public JavaFileObject getJavaFileForInput(Location location, String className, JavaFileObject.Kind kind)
                    throws IOException {
                return wrap(location, super.getJavaFileForInput(location, className, kind));
            }

            @Override
            public String inferBinaryName(Location location, JavaFileObject file) {
                return super.inferBinaryName(location, unwrap(file));
            }

            @Override
            public boolean isSameFile(FileObject a, FileObject b) {
                return super.isSameFile(unwrap(a), unwrap(b));
            }

            @Override
            public boolean contains(Location location, FileObject file) throws IOException {
                return super.contains(location, unwrap(file));
            }
        };
    }

    private static <T extends FileObject> T unwrap(T file) {
        @SuppressWarnings("unchecked")
        T inner = file instanceof Patched p ? (T) p.original() : file;
        return inner;
    }

    /** A library's source file as a patch changes it; its name is the library file's. */
    private static final class Patched extends ForwardingJavaFileObject<JavaFileObject> {
        private final Path patch;
        private final String binary;
        private String text;

        Patched(JavaFileObject original, Path patch, String binary) {
            super(original);
            this.patch = patch;
            this.binary = binary;
        }

        JavaFileObject original() {
            return fileObject;
        }

        @Override
        public CharSequence getCharContent(boolean ignoreEncodingErrors) throws IOException {
            if (text == null) {
                String original = fileObject.getCharContent(ignoreEncodingErrors).toString();
                try {
                    text = apply(patch.toString(), Files.readString(patch), fileObject.getName(), original);
                } catch (Refused e) {
                    throw new IOException(e.getMessage());
                }
                String show = System.getenv(SHOW);
                if (show != null && !show.isEmpty()) {
                    Path out = Path.of(show).resolve(binary.replace('.', '/') + ".java");
                    Files.createDirectories(out.getParent());
                    Files.writeString(out, text);
                }
            }
            return text;
        }

        @Override
        public InputStream openInputStream() throws IOException {
            return new ByteArrayInputStream(getCharContent(false).toString().getBytes(StandardCharsets.UTF_8));
        }

        @Override
        public Reader openReader(boolean ignoreEncodingErrors) throws IOException {
            return new StringReader(getCharContent(ignoreEncodingErrors).toString());
        }
    }

    /** The text of the library's file {@code originalName} as the patch {@code patchName} changes it. */
    static String apply(String patchName, String patchText, String originalName, String originalText) throws Refused {
        Parsed patch = Parsed.of(patchName, patchText);
        Parsed original = Parsed.of(originalName, originalText);
        return new JdkPatch(patch, original).run();
    }

    private final Parsed patch;
    private final Parsed original;
    /** The changes to the library's text: start, end, and the text that replaces that range. */
    private final List<Change> changes = new ArrayList<>();

    private record Change(long start, long end, String text, int order) {}

    /** The keys of the members of the class being patched that a patch has changed. */
    private Set<String> done = new HashSet<>();

    private JdkPatch(Parsed patch, Parsed original) {
        this.patch = patch;
        this.original = original;
    }

    private Refused refuse(Tree at, String message) {
        long line = at == null ? 0 : patch.unit.getLineMap().getLineNumber(patch.start(at));
        return new Refused(patch.name + ":" + line + ": " + message + " (patching " + original.name + ")");
    }

    private void change(long start, long end, String text) {
        changes.add(new Change(start, end, text, changes.size()));
    }

    private String run() throws Refused {
        String patchPackage = String.valueOf(patch.unit.getPackageName());
        String originalPackage = String.valueOf(original.unit.getPackageName());
        if (!patchPackage.equals(originalPackage)) {
            throw refuse(patch.unit.getPackage(), "the patch is of package " + patchPackage + ", the file of " + originalPackage);
        }
        imports();
        for (Tree decl : patch.unit.getTypeDecls()) {
            if (!(decl instanceof ClassTree patchClass)) {
                continue;
            }
            for (AnnotationTree marker : markers(patchClass.getModifiers())) {
                if (!name(marker).equals("Edit")) {
                    throw refuse(marker, "a patch's top-level class can only have @Edit");
                }
            }
            ClassTree originalClass = original.unit.getTypeDecls().stream()
                    .filter(t -> t instanceof ClassTree c && c.getSimpleName().contentEquals(patchClass.getSimpleName()))
                    .map(t -> (ClassTree) t).findFirst()
                    .orElseThrow(() -> refuse(patchClass, "the file has no class " + patchClass.getSimpleName()));
            patchClass(patchClass, originalClass);
        }
        return applyChanges();
    }

    /** Adds the patch's imports that the file does not have after the file's last import. */
    private void imports() {
        Set<String> have = new HashSet<>();
        for (ImportTree i : original.unit.getImports()) {
            have.add(squash(i.toString()));
        }
        StringBuilder added = new StringBuilder();
        for (ImportTree i : patch.unit.getImports()) {
            if (have.add(squash(i.toString()))) {
                added.append('\n').append(patch.text(i));
            }
        }
        if (added.isEmpty()) {
            return;
        }
        List<? extends ImportTree> imports = original.unit.getImports();
        long at = !imports.isEmpty() ? original.end(imports.get(imports.size() - 1))
                : original.unit.getPackage() != null ? original.end(original.unit.getPackage()) : 0;
        change(at, at, added.toString());
    }

    /** Applies the patch's class {@code patchClass} to the file's class {@code originalClass}. */
    private void patchClass(ClassTree patchClass, ClassTree originalClass) throws Refused {
        for (AnnotationTree marker : markers(patchClass.getModifiers())) {
            if (name(marker).equals("Edit")) {
                edit(marker, originalClass);
            }
        }
        Map<String, Tree> members = new LinkedHashMap<>();
        for (Tree member : originalClass.getMembers()) {
            String key = key(member);
            if (key != null) {
                members.putIfAbsent(key, member);
            }
        }
        Set<String> outer = done;
        done = new HashSet<>();
        StringBuilder added = new StringBuilder();
        for (Tree member : patchClass.getMembers()) {
            if (member instanceof BlockTree block) {
                initializer(block, originalClass);
                continue;
            }
            String key = key(member);
            if (key == null) {
                continue;
            }
            List<AnnotationTree> markers = markers(modifiers(member));
            Set<String> kinds = new LinkedHashSet<>();
            markers.forEach(m -> kinds.add(name(m)));
            String shown = shown(member, patchClass);
            if (kinds.isEmpty()) {
                throw refuse(member, shown + " says nothing: mark it @Replace, @Add, @Remove, @Edit or @Patch");
            }
            if (kinds.size() > 1) {
                throw refuse(member, shown + " has more than one of @Replace, @Add, @Remove, @Edit and @Patch");
            }
            String kind = kinds.iterator().next();
            Tree target = members.get(key);
            if (kind.equals("Add")) {
                if (target != null) {
                    throw refuse(member, shown + " is added, but the class has it already: @Replace it instead");
                }
                added.append("\n\n    ").append(textWithout(member, markers));
                continue;
            }
            if (target == null) {
                throw refuse(member, shown + " (@" + kind + ") is not in the class" + similar(member, members));
            }
            if (!done.add(key)) {
                throw refuse(member, shown + " is patched twice");
            }
            switch (kind) {
                case "Replace" -> change(original.start(target), original.end(target), textWithout(member, markers));
                case "Remove" -> change(original.start(target), original.end(target), "");
                case "Edit" -> {
                    for (AnnotationTree marker : markers) {
                        edit(marker, target);
                    }
                }
                case "Patch" -> {
                    if (!(member instanceof ClassTree nested) || !(target instanceof ClassTree nestedOriginal)) {
                        throw refuse(member, "@Patch is for nested classes");
                    }
                    patchClass(nested, nestedOriginal);
                }
                default -> throw new IllegalStateException(kind);
            }
        }
        done = outer;
        if (!added.isEmpty()) {
            long close = original.end(originalClass) - 1;
            if (original.text.charAt((int) close) != '}') {
                throw refuse(patchClass, "cannot find the end of the class " + originalClass.getSimpleName());
            }
            change(close, close, added.append('\n').toString());
        }
    }

    /**
     * Replaces the class's only initializer block of the same kind (static or not) with the
     * patch's {@code block}; an empty block removes the class's.
     */
    private void initializer(BlockTree block, ClassTree originalClass) throws Refused {
        String kind = block.isStatic() ? "static initializer" : "instance initializer";
        List<BlockTree> blocks = originalClass.getMembers().stream()
                .filter(m -> m instanceof BlockTree b && b.isStatic() == block.isStatic()).map(m -> (BlockTree) m).toList();
        if (blocks.size() != 1) {
            throw refuse(block, "the class " + originalClass.getSimpleName() + " has " + blocks.size() + " " + kind
                    + " blocks, not one to replace");
        }
        if (!done.add(kind)) {
            throw refuse(block, "the " + kind + " is patched twice");
        }
        String text = block.getStatements().isEmpty() ? "" : patch.text(block);
        change(original.start(blocks.get(0)), original.end(blocks.get(0)), text);
    }

    /**
     * Applies {@code @Edit} to the text of the file's {@code target}: {@code find}, which must be
     * there once, or the text from {@code from}, which must be there once, through the first
     * {@code to} after it, becomes {@code replace}.
     */
    private void edit(AnnotationTree marker, Tree target) throws Refused {
        Map<String, String> values = new HashMap<>();
        for (ExpressionTree argument : marker.getArguments()) {
            if (!(argument instanceof AssignmentTree a) || !(a.getVariable() instanceof IdentifierTree name)) {
                throw refuse(marker, "@Edit takes named values: find or from and to, and replace");
            }
            String value = string(a.getExpression());
            if (value == null) {
                throw refuse(marker, "@Edit takes string literals");
            }
            if (!Set.of("find", "from", "to", "replace").contains(name.getName().toString())) {
                throw refuse(marker, "@Edit takes find or from and to, and replace, not " + name.getName());
            }
            values.put(name.getName().toString(), value);
        }
        String replace = values.get("replace");
        String find = values.getOrDefault("find", values.get("from"));
        String to = values.get("to");
        boolean range = values.containsKey("from");
        if (replace == null || find == null || find.isBlank() || values.containsKey("find") == range
                || range != (to != null) || (range && to.isBlank())) {
            throw refuse(marker, "@Edit needs replace, and find or from and to");
        }
        int start = (int) original.start(target);
        int end = (int) original.end(target);
        Matcher m = pattern(find).matcher(original.text).region(start, end);
        if (!m.find()) {
            throw refuse(marker, "the text \"" + find + "\" is not in " + describe(target));
        }
        int at = m.start();
        int until = m.end();
        if (m.find()) {
            throw refuse(marker, "the text \"" + find + "\" is in " + describe(target) + " more than once");
        }
        if (range) {
            Matcher last = pattern(to).matcher(original.text).region(until, end);
            if (!last.find()) {
                throw refuse(marker, "the text \"" + to + "\" is not after \"" + find + "\" in " + describe(target));
            }
            until = last.end();
        }
        change(at, until, indented(replace, at));
    }

    /** {@code text} with each line after its first indented as the file's line at {@code at}. */
    private String indented(String text, int at) {
        int line = original.text.lastIndexOf('\n', at - 1) + 1;
        int code = line;
        while (code < at && Character.isWhitespace(original.text.charAt(code))) {
            code++;
        }
        String indent = original.text.substring(line, code);
        return text.stripTrailing().replace("\n", "\n" + indent).replaceAll("(?m)[ \t]+$", "");
    }

    /** The pattern of {@code text}, where a run of whitespace matches any run of whitespace. */
    private static Pattern pattern(String text) {
        StringBuilder regex = new StringBuilder();
        for (String piece : text.strip().split("\\s+")) {
            if (!regex.isEmpty()) {
                regex.append("\\s+");
            }
            regex.append(Pattern.quote(piece));
        }
        return Pattern.compile(regex.toString());
    }

    /** The value of a string literal, or of string literals joined with {@code +}. */
    private static String string(ExpressionTree e) {
        if (e instanceof LiteralTree l && l.getValue() instanceof String s) {
            return s;
        }
        if (e instanceof ParenthesizedTree p) {
            return string(p.getExpression());
        }
        if (e instanceof BinaryTree b && b.getKind() == Tree.Kind.PLUS) {
            String left = string(b.getLeftOperand());
            String right = string(b.getRightOperand());
            return left == null || right == null ? null : left + right;
        }
        return null;
    }

    private String describe(Tree target) {
        return target instanceof ClassTree c ? "the class " + c.getSimpleName() : shown(target, null);
    }

    /** The members of the file's class with the same name as {@code member}, as a hint. */
    private String similar(Tree member, Map<String, Tree> members) {
        String key = key(member);
        String name = key.contains("(") ? key.substring(0, key.indexOf('(') + 1) : key;
        List<String> same = members.keySet().stream().filter(k -> k.startsWith(name)).toList();
        return same.isEmpty() ? "" : "; it has " + String.join(", ", same);
    }

    /** The text of the patch's {@code member} without its marker annotations. */
    private String textWithout(Tree member, List<AnnotationTree> markers) {
        long start = patch.start(member);
        StringBuilder text = new StringBuilder(patch.text.substring((int) start, (int) patch.end(member)));
        List<AnnotationTree> sorted = new ArrayList<>(markers);
        sorted.sort(Comparator.comparingLong(m -> -patch.start(m)));
        for (AnnotationTree marker : sorted) {
            int from = (int) (patch.start(marker) - start);
            int to = (int) (patch.end(marker) - start);
            while (to < text.length() && Character.isWhitespace(text.charAt(to))) {
                to++;
            }
            text.delete(from, to);
        }
        return text.toString();
    }

    /** The text of the file with the changes made, refused if two changes overlap. */
    private String applyChanges() throws Refused {
        changes.sort(Comparator.comparingLong(Change::start).thenComparingInt(Change::order));
        StringBuilder out = new StringBuilder();
        long at = 0;
        for (Change c : changes) {
            if (c.start < at) {
                throw refuse(null, "two changes overlap at line " + original.unit.getLineMap().getLineNumber(c.start));
            }
            out.append(original.text, (int) at, (int) c.start).append(c.text);
            at = c.end;
        }
        out.append(original.text, (int) at, original.text.length());
        return out.toString();
    }

    /** The marker annotations among {@code modifiers}'. */
    private static List<AnnotationTree> markers(ModifiersTree modifiers) {
        List<AnnotationTree> out = new ArrayList<>();
        if (modifiers != null) {
            for (AnnotationTree a : modifiers.getAnnotations()) {
                if (MARKERS.contains(name(a))) {
                    out.add(a);
                }
            }
        }
        return out;
    }

    private static String name(AnnotationTree a) {
        return a.getAnnotationType().toString();
    }

    private static ModifiersTree modifiers(Tree member) {
        return switch (member) {
            case MethodTree m -> m.getModifiers();
            case VariableTree v -> v.getModifiers();
            case ClassTree c -> c.getModifiers();
            default -> null;
        };
    }

    /**
     * The key that a member is matched by: a method's or constructor's name ({@code <init>}) and
     * parameter types as written, a field's name, or a nested class's; null for an initializer.
     */
    private static String key(Tree member) {
        return switch (member) {
            case MethodTree m -> m.getName() + "(" + String.join(",",
                    m.getParameters().stream().map(p -> squash(p.getType().toString())).toList()) + ")";
            case VariableTree v -> v.getName().toString();
            case ClassTree c -> "class " + c.getSimpleName();
            default -> null;
        };
    }

    /** A member as an error names it. */
    private static String shown(Tree member, ClassTree owner) {
        String key = key(member);
        if (member instanceof MethodTree m && m.getName().contentEquals("<init>") && owner != null) {
            return "the constructor " + owner.getSimpleName() + key.substring("<init>".length());
        }
        return switch (member) {
            case MethodTree m -> "the method " + key;
            case VariableTree v -> "the field " + key;
            default -> "the " + key;
        };
    }

    /** {@code s} without whitespace. */
    private static String squash(String s) {
        return s.replaceAll("\\s+", "");
    }

    /** A file parsed by javac, with the positions of its trees. */
    private record Parsed(String name, String text, CompilationUnitTree unit, SourcePositions positions) {
        static Parsed of(String name, String text) throws Refused {
            JavaFileObject file = new SimpleJavaFileObject(URI.create("string:///Patched.java"), JavaFileObject.Kind.SOURCE) {
                @Override
                public CharSequence getCharContent(boolean ignoreEncodingErrors) {
                    return text;
                }
            };
            DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
            JavacTask task = (JavacTask) ToolProvider.getSystemJavaCompiler().getTask(null, null, diagnostics,
                    List.of("-proc:none"), null, List.of(file));
            CompilationUnitTree unit;
            try {
                unit = task.parse().iterator().next();
            } catch (IOException e) {
                throw new Refused(name + ": " + e.getMessage());
            }
            for (Diagnostic<? extends JavaFileObject> d : diagnostics.getDiagnostics()) {
                if (d.getKind() == Diagnostic.Kind.ERROR) {
                    throw new Refused(name + ":" + d.getLineNumber() + ": " + d.getMessage(Locale.ROOT));
                }
            }
            return new Parsed(name, text, unit, Trees.instance(task).getSourcePositions());
        }

        long start(Tree tree) {
            return positions.getStartPosition(unit, tree);
        }

        /** The end of a tree, with the semicolon that ends a field's declaration. */
        long end(Tree tree) {
            long end = positions.getEndPosition(unit, tree);
            if (tree instanceof VariableTree && text.charAt((int) end - 1) != ';') {
                int at = (int) end;
                while (at < text.length() && Character.isWhitespace(text.charAt(at))) {
                    at++;
                }
                if (at < text.length() && text.charAt(at) == ';') {
                    end = at + 1;
                }
            }
            return end;
        }

        String text(Tree tree) {
            return text.substring((int) start(tree), (int) end(tree));
        }
    }
}
