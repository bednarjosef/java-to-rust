import com.sun.source.tree.CaseTree;
import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.LambdaExpressionTree;
import com.sun.source.tree.MemberReferenceTree;
import com.sun.source.tree.MemberSelectTree;
import com.sun.source.tree.MethodInvocationTree;
import com.sun.source.tree.Tree;
import com.sun.source.util.TreePath;
import com.sun.source.util.TreePathScanner;
import com.sun.tools.javac.api.ClientCodeWrapper;
import com.sun.tools.javac.code.Kinds;
import com.sun.tools.javac.code.Symbol;
import com.sun.tools.javac.code.Symbol.ClassSymbol;
import com.sun.tools.javac.code.Symbol.MethodSymbol;
import com.sun.tools.javac.code.Symbol.PackageSymbol;
import com.sun.tools.javac.code.Symtab;
import com.sun.tools.javac.code.Type;
import com.sun.tools.javac.code.TypeTag;
import com.sun.tools.javac.tree.JCTree;
import com.sun.tools.javac.tree.JCTree.*;
import com.sun.tools.javac.tree.TreeInfo;
import com.sun.tools.javac.tree.TreeMaker;
import com.sun.tools.javac.util.Context;
import com.sun.tools.javac.util.JCDiagnostic;
import com.sun.tools.javac.util.Name;
import com.sun.tools.javac.util.Names;
import java.net.URI;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.Map;
import java.util.Set;
import javax.lang.model.util.Elements;
import javax.tools.Diagnostic;
import javax.tools.JavaFileObject;

/**
 * Code that names a class the source path does not have, made to fail where the JVM fails when
 * such a class is missing at run time, instead of being rejected (with {@code --keep-going}). A
 * library's optional dependency is such a class: the library checks whether it is there before
 * it uses it, and the code that uses it only fails if it runs.
 *
 * <p>javac reports each name of a missing class as a compile error ({@link #classify}): "package
 * does not exist" for a qualified name, "cannot find symbol" for a simple name that an import
 * binds to a class of a package that does not exist. The class's binary name is taken from the
 * qualified name, its package being the names before the first one that starts with an upper-case
 * letter. Where the name is a type of a declaration (a variable's, a method's result, a type
 * argument), it is a type that only {@code null} can have, which the trees name as
 * {@code java.lang.Object}, as the JVM loads no class for it. Where the code does something with
 * the class (calls a method of it, reads one of its fields, creates one of its objects, casts to
 * it, tests {@code instanceof} it, makes a lambda of it), or with a value of it, the JVM would
 * resolve the class there and throw {@code NoClassDefFoundError}: that code is replaced by
 * {@code throw new NoClassDefFoundError("binary/name")} ({@link #rewrite}). What is replaced is the
 * innermost piece around it that runs or not as a whole: a statement (with the statements after
 * it in its block, which cannot run), the body of an {@code if}, a loop or a {@code case}, the
 * body of an expression lambda, a branch of {@code ?:}, the right side of {@code &&} or
 * {@code ||}, or a field's initializer; an expression is replaced by a {@code switch} whose only
 * case throws. A piece of code with any other compile error stays rejected.
 *
 * <p>A name of a missing class in an annotation is no error: the JVM ignores an annotation whose
 * class is missing, so the code it marks runs as if it were not there. Nor is an import of a
 * missing class that the unit's code uses only in annotations, or not at all ({@link #ignored}).
 * Those errors are dropped, without {@code --keep-going} too.
 *
 * <p>A class whose superclass or interface is missing, or that catches a missing class (whose
 * class the verifier loads), cannot be loaded at all: each of its methods and constructors
 * throws, and its initializers are left out, as they never run. Its static fields can still be
 * read and written, where the JVM would throw.
 *
 * <p>Other deliberate differences: the code throws at the start of the piece, not at the
 * instruction that uses the class, so what the piece does before that use is not done; a cast
 * or {@code instanceof} of {@code null}, which the JVM passes without loading the class, throws;
 * a constant of a missing class, which javac would have inlined, throws; a method reference to a
 * class that cannot be loaded is made without throwing; and the binary name is a guess when a
 * class is nested in a package whose names start with upper-case letters.
 */
final class MissingClasses {
    /** Where a compile error names a missing class. */
    enum Place {
        /** In an import, which marks nothing: the code that uses the name has errors of its own. */
        IMPORT,
        /** In an annotation, which the JVM ignores when its class is missing ({@link #ignored}). */
        ANNOTATION,
        /** As the type of a declaration, or a type argument: no class is loaded for it. */
        TYPE,
        /** Where code does something with the class, which the JVM resolves there. */
        OPERATION,
        /**
         * As the superclass or an interface of a class, or in a {@code catch} of its code, whose
         * class the JVM's verifier loads: the class cannot be loaded.
         */
        UNLOADABLE,
    }

    /**
     * A compile error that names a missing class: the class's binary name, where the code names
     * it, the path to the name, and for {@link Place#UNLOADABLE} the class that cannot be loaded.
     */
    record Missing(String binary, Place place, TreePath path, JCClassDecl inClass) {}

    private final Elements elements;
    private final TreeMaker make;
    private final Symtab syms;
    private final Names names;
    /** The units javac parsed, by file. */
    private final Map<URI, JCCompilationUnit> units = new HashMap<>();
    /** Variables and methods declared with a missing class as their type or result. */
    private final Map<Symbol, String> declared = new IdentityHashMap<>();
    /** The error types javac made for the names of missing classes, by their symbols. */
    private final Map<Symbol, String> errorTypes = new IdentityHashMap<>();
    /** The simple names that units use outside imports and annotations, once needed ({@link #ignored}). */
    private final Map<JCCompilationUnit, Set<Name>> usedNames = new IdentityHashMap<>();
    /** {@code java.lang.NoClassDefFoundError} and its constructor of a message, once needed. */
    private ClassSymbol error;
    private MethodSymbol errorOfMessage;

    MissingClasses(Context context, Elements elements) {
        this.elements = elements;
        this.make = TreeMaker.instance(context);
        this.syms = Symtab.instance(context);
        this.names = Names.instance(context);
    }

    /** Keeps a unit that javac parsed, whose imports bind the names in its compile errors. */
    void parsed(CompilationUnitTree unit) {
        units.put(unit.getSourceFile().toUri(), (JCCompilationUnit) unit);
    }

    /** Whether any compile error so far named a missing class. */
    boolean any() {
        return !declared.isEmpty() || !errorTypes.isEmpty();
    }

    /** The missing class that a compile error names, or null for any other error. */
    Missing classify(Diagnostic<? extends JavaFileObject> d) {
        JCDiagnostic diagnostic = d instanceof JCDiagnostic j ? j
                : d instanceof ClientCodeWrapper.DiagnosticSourceUnwrapper u ? u.d : null;
        if (diagnostic == null || d.getSource() == null || diagnostic.getDiagnosticPosition() == null) {
            return null;
        }
        JCCompilationUnit unit = units.get(d.getSource().toUri());
        JCTree tree = diagnostic.getDiagnosticPosition().getTree();
        if (unit == null || tree == null) {
            return null;
        }
        String code = diagnostic.getCode();
        String binary = null;
        if (code.equals("compiler.err.static.imp.only.classes.and.interfaces") && tree instanceof JCImport imp
                && imp.qualid != null && missingClass(imp.qualid.selected)) {
            // A static import of a missing class, whose name javac took as a package's.
            TreePath path = TreePath.getPath(unit, tree);
            return path == null ? null
                    : new Missing(binaryName(TreeInfo.fullName(imp.qualid.selected).toString()), Place.IMPORT, path, null);
        }
        if (code.equals("compiler.err.doesnt.exist") && tree instanceof JCFieldAccess select
                && select.name != names.asterisk) {
            binary = qualified(unit, select);
        } else if (code.startsWith("compiler.err.cant.resolve") && tree instanceof JCIdent ident) {
            Object[] args = diagnostic.getArgs();
            boolean member = args.length > 0 && (args[0] == Kinds.KindName.METHOD || args[0] == Kinds.KindName.CONSTRUCTOR);
            binary = imported(unit, ident.name, member);
        } else if (code.startsWith("compiler.err.cant.resolve") && tree instanceof JCFieldAccess select
                && select.selected instanceof JCIdent qualifier) {
            // A member of a class that a single import names, which is missing: javac found
            // another class of the same name instead (of the unit's package, say), which the
            // import hides in Java.
            binary = singleImport(unit, qualifier.name);
        }
        if (binary == null) {
            return null;
        }
        TreePath path = TreePath.getPath(unit, tree);
        if (path == null) {
            return null;
        }
        if (tree instanceof JCIdent || code.equals("compiler.err.doesnt.exist")) {
            noteType(tree.type, binary);
        }
        return place(path, binary);
    }

    /** Where the name at {@code path} is, recording the declarations it is the type of. */
    private Missing place(TreePath path, String binary) {
        // In an annotation's type or in its arguments (`@Support(level = Level.FULL)`).
        for (TreePath at = path; at != null; at = at.getParentPath()) {
            if (at.getLeaf() instanceof JCAnnotation) {
                return new Missing(binary, Place.ANNOTATION, path, null);
            }
        }
        boolean argument = false;
        boolean array = false;
        for (TreePath from = path, at = path.getParentPath(); at != null; from = at, at = at.getParentPath()) {
            JCTree parent = (JCTree) at.getLeaf();
            JCTree child = (JCTree) from.getLeaf();
            switch (parent) {
                case JCImportBase i -> {
                    return new Missing(binary, Place.IMPORT, path, null);
                }
                case JCFieldAccess select -> noteType(select.type, binary);
                case JCTypeApply apply -> argument |= apply.clazz != child;
                case JCArrayTypeTree a -> array = true;
                case JCWildcard w -> argument = true;
                case JCAnnotatedType a -> {}
                case JCTypeUnion u -> {}
                case JCTypeIntersection i -> {}
                case JCVariableDecl variable when variable.vartype == child -> {
                    if (at.getParentPath().getLeaf() instanceof JCCatch) {
                        return new Missing(binary, Place.UNLOADABLE, path, enclosingClass(at));
                    }
                    if (!argument && !array && variable.sym != null) {
                        declared.put(variable.sym, binary);
                    }
                    return new Missing(binary, Place.TYPE, path, null);
                }
                case JCMethodDecl method -> {
                    if (method.restype == child && !argument && !array && method.sym != null) {
                        declared.put(method.sym, binary);
                    }
                    return new Missing(binary, Place.TYPE, path, null);
                }
                case JCTypeParameter p -> {
                    return new Missing(binary, Place.TYPE, path, null);
                }
                case JCClassDecl c -> {
                    boolean supertype = !argument && (c.extending == child || c.implementing.contains(child));
                    return new Missing(binary, supertype ? Place.UNLOADABLE : Place.TYPE, path, c);
                }
                default -> {
                    return new Missing(binary, argument ? Place.TYPE : Place.OPERATION, path, null);
                }
            }
        }
        return new Missing(binary, Place.TYPE, path, null);
    }

    /**
     * Whether a compile error that names a missing class is one that the JVM has no counterpart
     * of, which is no error at all: the name is in an annotation, which the JVM ignores when the
     * annotation's class is missing (as the compile-only annotations of a library are, such as
     * Error Prone's), or in a single import whose name no code of the unit uses outside
     * annotations.
     */
    boolean ignored(Missing m) {
        if (m.place() == Place.ANNOTATION) {
            return true;
        }
        if (m.place() != Place.IMPORT) {
            return false;
        }
        TreePath at = m.path();
        while (at != null && !(at.getLeaf() instanceof JCImport)) {
            at = at.getParentPath();
        }
        if (at == null || !(at.getLeaf() instanceof JCImport imp) || imp.qualid == null
                || imp.qualid.name == names.asterisk) {
            return false;
        }
        JCCompilationUnit unit = (JCCompilationUnit) at.getCompilationUnit();
        return !usedNames.computeIfAbsent(unit, MissingClasses::namesUsed).contains(imp.qualid.name);
    }

    /** The simple names that the code of a unit uses outside imports and annotations. */
    private static Set<Name> namesUsed(JCCompilationUnit unit) {
        Set<Name> used = new java.util.HashSet<>();
        new com.sun.tools.javac.tree.TreeScanner() {
            @Override
            public void visitImport(JCImport tree) {}

            @Override
            public void visitAnnotation(JCAnnotation tree) {}

            @Override
            public void visitIdent(JCIdent tree) {
                used.add(tree.name);
            }
        }.scan(unit);
        return used;
    }

    /** The innermost class around {@code path}. */
    private static JCClassDecl enclosingClass(TreePath path) {
        for (TreePath at = path; at != null; at = at.getParentPath()) {
            if (at.getLeaf() instanceof JCClassDecl c) {
                return c;
            }
        }
        return null;
    }

    /** Records that {@code type}, when javac made it for a name it could not resolve, is the missing class. */
    private void noteType(Type type, String binary) {
        if (type != null && type.hasTag(TypeTag.ERROR) && type.tsym != syms.errSymbol) {
            errorTypes.putIfAbsent(type.tsym, binary);
        }
    }

    /**
     * The binary name of the class that a qualified name names whose qualifier javac found to be
     * no package: {@code a.b.C}, or {@code Outer.Inner} when an import binds {@code Outer} to a
     * missing class.
     */
    private String qualified(JCCompilationUnit unit, JCFieldAccess select) {
        String qualifier = TreeInfo.fullName(select.selected).toString();
        String first = qualifier.contains(".") ? qualifier.substring(0, qualifier.indexOf('.')) : qualifier;
        String outer = imported(unit, names.fromString(first), false);
        if (outer != null) {
            String rest = qualifier.substring(first.length()).replace('.', '$');
            return outer + rest + "$" + select.name;
        }
        return binaryName(qualifier + "." + select.name);
    }

    /**
     * The binary name of a dotted class name whose package does not exist: the package is the
     * names before the first that starts with an upper-case letter (or all but the last), the
     * rest are a class and the classes nested in it.
     */
    private static String binaryName(String dotted) {
        String[] parts = dotted.split("\\.");
        int start = parts.length - 1;
        for (int i = 0; i < parts.length; i++) {
            if (!parts[i].isEmpty() && Character.isUpperCase(parts[i].charAt(0))) {
                start = i;
                break;
            }
        }
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < parts.length; i++) {
            if (i > 0) {
                out.append(i <= start ? '/' : '$');
            }
            out.append(parts[i]);
        }
        return out.toString();
    }

    /**
     * The missing class that a simple name of a unit names through its imports: a class's name
     * (or, when {@code member}, a method's) that a single import binds to a missing class, or else
     * the one import on demand of a missing package or class that could bind it. Null when no
     * import binds it so.
     */
    private String imported(JCCompilationUnit unit, Name name, boolean member) {
        java.util.List<String> candidates = new ArrayList<>();
        for (JCImportBase base : unit.getImports()) {
            if (!(base instanceof JCImport imp) || imp.qualid == null) {
                continue;
            }
            JCFieldAccess qualid = imp.qualid;
            boolean onDemand = qualid.name == names.asterisk;
            if (!imp.staticImport) {
                if (member) {
                    continue;
                }
                if (!onDemand && qualid.name == name && missingClass(qualid)) {
                    return binaryName(TreeInfo.fullName(qualid).toString());
                }
                if (onDemand && missingPackage(qualid.selected)) {
                    candidates.add(TreeInfo.fullName(qualid.selected).toString().replace('.', '/') + "/" + name);
                }
            } else if (missingClass(qualid.selected)) {
                String owner = binaryName(TreeInfo.fullName(qualid.selected).toString());
                if (!onDemand && qualid.name == name) {
                    return owner;
                }
                if (onDemand) {
                    candidates.add(owner);
                }
            }
        }
        return candidates.size() == 1 ? candidates.get(0) : null;
    }

    /** The missing class that a single import (not static, not on demand) binds {@code name} to, if one does. */
    private String singleImport(JCCompilationUnit unit, Name name) {
        for (JCImportBase base : unit.getImports()) {
            if (base instanceof JCImport imp && !imp.staticImport && imp.qualid != null
                    && imp.qualid.name == name && missingClass(imp.qualid)) {
                return binaryName(TreeInfo.fullName(imp.qualid).toString());
            }
        }
        return null;
    }

    /** Whether the class that a name of an import names is missing. */
    private boolean missingClass(JCExpression name) {
        if (name.type != null) {
            return name.type.isErroneous();
        }
        if (TreeInfo.symbol(name) instanceof PackageSymbol) {
            return false;
        }
        return elements.getTypeElement(TreeInfo.fullName(name).toString()) == null;
    }

    /** Whether the package that an import on demand names does not exist. */
    private boolean missingPackage(JCExpression name) {
        return TreeInfo.symbol(name) instanceof PackageSymbol p && !p.exists();
    }

    /**
     * The missing class that a value of {@code expression} is of, when it is one: one whose type
     * is the error type javac made for a missing class, or which a declaration of a missing class
     * gives (a variable, a method's result).
     */
    private String valueOf(JCExpression expression, JCCompilationUnit unit) {
        if (expression == null) {
            return null;
        }
        String typed = ofType(expression.type, unit);
        if (typed != null) {
            return typed;
        }
        JCExpression named = expression instanceof JCMethodInvocation call ? call.meth : expression;
        Symbol symbol = TreeInfo.symbol(named);
        return symbol == null ? null : declared.get(symbol);
    }

    /**
     * The missing class that the lambda or method reference at {@code path} is an object of: its
     * type's, or that of what it is given to (a variable, a parameter, a method's result).
     */
    private String functional(TreePath path, JCCompilationUnit unit) {
        JCExpression tree = (JCExpression) path.getLeaf();
        String typed = ofType(tree.type, unit);
        if (typed != null || tree.type == null || !tree.type.isErroneous()) {
            return typed;
        }
        TreePath from = path;
        TreePath at = path.getParentPath();
        while (at != null && (at.getLeaf() instanceof JCParens || at.getLeaf() instanceof JCConditional)) {
            from = at;
            at = at.getParentPath();
        }
        if (at == null) {
            return null;
        }
        JCTree child = (JCTree) from.getLeaf();
        Symbol target = switch ((JCTree) at.getLeaf()) {
                case JCVariableDecl variable -> variable.sym;
                case JCAssign assign -> TreeInfo.symbol(assign.lhs);
                case JCReturn r -> enclosingMethod(at);
                case JCMethodInvocation call when TreeInfo.symbol(call.meth) instanceof MethodSymbol m
                        && call.args.indexOf(child) >= 0 && call.args.indexOf(child) < m.params().size() ->
                    m.params().get(call.args.indexOf(child));
                case JCNewClass created when created.constructor instanceof MethodSymbol m
                        && created.args.indexOf(child) >= 0 && created.args.indexOf(child) < m.params().size() ->
                    m.params().get(created.args.indexOf(child));
                default -> null;
            };
        return target == null ? null : declared.get(target);
    }

    /** The method (or lambda's method, for which this is null) that a {@code return} at {@code path} returns from. */
    private static Symbol enclosingMethod(TreePath path) {
        for (TreePath at = path; at != null; at = at.getParentPath()) {
            if (at.getLeaf() instanceof JCLambda) {
                return null;
            }
            if (at.getLeaf() instanceof JCMethodDecl method) {
                return method.sym;
            }
        }
        return null;
    }

    /** The missing class that {@code type} is, when it is an error type made for one. */
    private String ofType(Type type, JCCompilationUnit unit) {
        if (type == null || !type.hasTag(TypeTag.ERROR)) {
            return null;
        }
        for (Type at = type; at != null && at.hasTag(TypeTag.ERROR); ) {
            String binary = at.tsym == syms.errSymbol ? null : errorTypes.get(at.tsym);
            if (binary != null) {
                return binary;
            }
            Type original = at instanceof Type.ErrorType e ? e.getOriginalType() : null;
            at = original == at ? null : original;
        }
        return type.tsym == syms.errSymbol ? null : imported(unit, type.tsym.name, false);
    }

    /**
     * What a unit does with missing classes, before it is rewritten ({@link #rewrite}): the code
     * that does something with one, and the classes that cannot be loaded, with the first missing
     * class each names.
     */
    record Plan(java.util.List<Op> ops, java.util.List<Missing> unloadable) {}

    /** What a unit does with missing classes, given its compile errors that name them. */
    Plan plan(JCCompilationUnit unit, java.util.List<Missing> errors) {
        java.util.List<Op> ops = new ArrayList<>();
        java.util.List<Missing> unloadable = new ArrayList<>();
        for (Missing m : errors) {
            if (m.place() == Place.UNLOADABLE && m.inClass() != null) {
                unloadable.add(m);
            } else if (m.place() == Place.OPERATION) {
                ops.add(new Op(m.path(), m.binary()));
            }
        }
        if (any()) {
            new TreePathScanner<Void, Void>() {
                @Override
                public Void visitAnnotation(com.sun.source.tree.AnnotationTree tree, Void unused) {
                    return null;
                }

                @Override
                public Void visitMethodInvocation(MethodInvocationTree tree, Void unused) {
                    if (((JCMethodInvocation) tree).meth instanceof JCFieldAccess select) {
                        found(valueOf(select.selected, unit));
                    }
                    return super.visitMethodInvocation(tree, unused);
                }

                @Override
                public Void visitMemberSelect(MemberSelectTree tree, Void unused) {
                    JCFieldAccess select = (JCFieldAccess) tree;
                    boolean called = getCurrentPath().getParentPath().getLeaf() instanceof JCMethodInvocation call
                            && call.meth == select;
                    // A class's name as the qualifier is a compile error of its own.
                    Symbol qualifier = TreeInfo.symbol(select.selected);
                    boolean value = qualifier == null || qualifier.kind == Kinds.Kind.VAR || qualifier.kind == Kinds.Kind.MTH;
                    if (!called && value && select.selected.type != null && !select.selected.type.hasTag(TypeTag.ARRAY)) {
                        found(valueOf(select.selected, unit));
                    }
                    return super.visitMemberSelect(tree, unused);
                }

                @Override
                public Void visitLambdaExpression(LambdaExpressionTree tree, Void unused) {
                    found(functional(getCurrentPath(), unit));
                    return super.visitLambdaExpression(tree, unused);
                }

                @Override
                public Void visitMemberReference(MemberReferenceTree tree, Void unused) {
                    found(functional(getCurrentPath(), unit));
                    return super.visitMemberReference(tree, unused);
                }

                private void found(String binary) {
                    if (binary != null) {
                        ops.add(new Op(getCurrentPath(), binary));
                    }
                }
            }.scan(unit, null);
        }
        return new Plan(ops, unloadable);
    }

    /**
     * Whether a compile error of a unit is one that only its missing classes cause: an exception
     * that "is never thrown" in a {@code try} whose code does something with a missing class,
     * which javac cannot tell what it throws.
     */
    boolean explains(Plan plan, Diagnostic<? extends JavaFileObject> d, JCCompilationUnit unit) {
        JCDiagnostic diagnostic = d instanceof JCDiagnostic j ? j
                : d instanceof ClientCodeWrapper.DiagnosticSourceUnwrapper u ? u.d : null;
        if (diagnostic == null || !diagnostic.getCode().equals("compiler.err.except.never.thrown.in.try")
                || diagnostic.getDiagnosticPosition() == null || diagnostic.getDiagnosticPosition().getTree() == null) {
            return false;
        }
        TreePath path = TreePath.getPath(unit, diagnostic.getDiagnosticPosition().getTree());
        while (path != null && !(path.getLeaf() instanceof JCTry)) {
            path = path.getParentPath();
        }
        if (path == null) {
            return false;
        }
        JCBlock body = ((JCTry) path.getLeaf()).body;
        for (Op op : plan.ops()) {
            for (TreePath at = op.path(); at != null; at = at.getParentPath()) {
                if (at.getLeaf() == body) {
                    return true;
                }
            }
        }
        return false;
    }

    /**
     * Rewrites the code of a unit that does something with a missing class ({@code plan}) to
     * throw {@code NoClassDefFoundError} as the JVM would there, before the unit is printed,
     * except in the pieces of code that other compile errors are in ({@code broken}), which are
     * rejected and left as they are. Returns the pieces of code of the unit's classes (methods,
     * field initializers and initializer blocks) that throw so, with the first missing class each
     * names.
     */
    Map<Tree, String> rewrite(Plan plan, Set<Tree> broken) {
        Map<Tree, String> pieces = new IdentityHashMap<>();
        // The classes that cannot be loaded throw in all their code.
        Map<JCClassDecl, Missing> unloadable = new IdentityHashMap<>();
        for (Missing m : plan.unloadable()) {
            if (!broken.contains(m.inClass())) {
                unloadable.putIfAbsent(m.inClass(), m);
            }
        }
        for (Map.Entry<JCClassDecl, Missing> e : unloadable.entrySet()) {
            unloadable(e.getKey(), e.getValue(), pieces);
        }
        java.util.List<Op> ops = new ArrayList<>(plan.ops());
        // Each operation replaces the code around it, the earliest one of each place first.
        ops.sort(Comparator.comparingInt(op -> TreeInfo.getStartPos((JCTree) op.path().getLeaf())));
        // By the block or case whose statements are replaced from one on, or else the code replaced.
        Map<JCTree, Replacement> replacements = new IdentityHashMap<>();
        for (Op op : ops) {
            Replacement r = replacement(op.path(), broken, unloadable.keySet());
            if (r == null) {
                continue;
            }
            JCTree key = r.index() >= 0 ? r.parent() : r.child();
            Replacement first = replacements.get(key);
            if (first == null || r.index() < first.index()) {
                replacements.put(key, r.with(op.binary()));
            }
            if (r.piece() != null) {
                pieces.putIfAbsent(r.piece(), op.binary());
            }
        }
        for (Replacement r : replacements.values()) {
            replace(r);
        }
        return pieces;
    }

    /** Code that does something with a missing class, at {@code path}. */
    private record Op(TreePath path, String binary) {}

    /**
     * The code that an operation replaces: the {@code child} of {@code parent}, which is the
     * statement at {@code index} of a block or a case's statements (-1 for any other place); the
     * piece of code it is in, and the missing class it throws for.
     */
    private record Replacement(JCTree parent, JCTree child, int index, Tree piece, String binary) {
        Replacement with(String binary) {
            return new Replacement(parent, child, index, piece, binary);
        }
    }

    /**
     * What an operation at {@code path} replaces, or null when it is in code that is rejected (in
     * {@code broken}) or in a class that cannot be loaded (in {@code unloadable}), or in no code
     * that can be replaced.
     */
    private Replacement replacement(TreePath path, Set<Tree> broken, Set<JCClassDecl> unloadable) {
        JCTree parent = null;
        JCTree child = null;
        int index = -1;
        Tree piece = null;
        for (TreePath from = path, at = path.getParentPath(); at != null; from = at, at = at.getParentPath()) {
            JCTree above = (JCTree) at.getLeaf();
            JCTree below = (JCTree) from.getLeaf();
            if (broken.contains(above) || unloadable.contains(above)) {
                return null;
            }
            if (piece == null && above instanceof JCClassDecl && member(at)) {
                piece = below;
            }
            if (parent != null) {
                continue;
            }
            int found = switch (above) {
                case JCBlock block -> block.stats.indexOf(below);
                case JCCase c when c.body == below -> -1;
                case JCCase c -> c.stats.indexOf(below);
                default -> -2;
            };
            boolean place = switch (above) {
                case JCBlock block -> found >= 0;
                case JCCase c -> found >= 0 || c.body == below;
                case JCIf i -> i.thenpart == below || i.elsepart == below;
                case JCWhileLoop loop -> loop.body == below;
                case JCDoWhileLoop loop -> loop.body == below;
                case JCForLoop loop -> loop.body == below;
                case JCEnhancedForLoop loop -> loop.body == below;
                case JCLabeledStatement labeled -> labeled.body == below;
                case JCLambda lambda -> lambda.body == below && below instanceof JCExpression;
                case JCConditional c -> c.truepart == below || c.falsepart == below;
                case JCBinary b -> (b.hasTag(JCTree.Tag.AND) || b.hasTag(JCTree.Tag.OR)) && b.rhs == below;
                case JCVariableDecl variable -> variable.init == below && at.getParentPath().getLeaf() instanceof JCClassDecl;
                default -> false;
            };
            if (place) {
                parent = above;
                child = below;
                index = Math.max(found, -1);
            } else if (above instanceof JCClassDecl || above instanceof JCMethodDecl) {
                return null;
            }
        }
        return parent == null ? null : new Replacement(parent, child, index, piece, null);
    }

    /** Whether the class at {@code path} is one whose members are pieces of code of their own: not local or anonymous. */
    private static boolean member(TreePath path) {
        Tree outer = path.getParentPath() == null ? null : path.getParentPath().getLeaf();
        return outer instanceof JCCompilationUnit || outer instanceof JCClassDecl;
    }

    /** Puts code that throws in place of the code that {@code r} replaces. */
    private void replace(Replacement r) {
        int pos = TreeInfo.getStartPos(r.child());
        JCThrow throwing = throwing(r.binary(), pos);
        switch (r.parent()) {
            case JCBlock block -> block.stats = block.stats.take(r.index()).append(throwing);
            case JCCase c when r.index() >= 0 -> {
                c.stats = c.stats.take(r.index()).append(throwing);
                if (c.caseKind == CaseTree.CaseKind.RULE) {
                    c.body = throwing;
                }
                c.completesNormally = false;
            }
            case JCCase c -> {
                c.stats = com.sun.tools.javac.util.List.of(throwing);
                c.body = throwing;
                c.completesNormally = false;
            }
            case JCIf i -> {
                if (i.thenpart == r.child()) {
                    i.thenpart = throwing;
                } else {
                    i.elsepart = throwing;
                }
            }
            case JCWhileLoop loop -> loop.body = throwing;
            case JCDoWhileLoop loop -> loop.body = throwing;
            case JCForLoop loop -> loop.body = throwing;
            case JCEnhancedForLoop loop -> loop.body = throwing;
            case JCLabeledStatement labeled -> labeled.body = throwing;
            case JCLambda lambda -> {
                lambda.body = make.at(pos).Block(0, com.sun.tools.javac.util.List.of(throwing));
                lambda.canCompleteNormally = false;
            }
            case JCVariableDecl variable -> variable.init = throwingValue(r.binary(), pos, variable.type);
            case JCConditional c when c.truepart == r.child() ->
                c.truepart = throwingValue(r.binary(), pos, branchType(c, c.truepart, c.falsepart));
            case JCConditional c -> c.falsepart = throwingValue(r.binary(), pos, branchType(c, c.falsepart, c.truepart));
            case JCBinary b -> b.rhs = throwingValue(r.binary(), pos, syms.booleanType);
            default -> throw new IllegalStateException("no code to replace in " + r.parent().getTag());
        }
    }

    /**
     * The type of the value that replaces {@code branch} of {@code c}: its own, {@code c}'s or the
     * other branch's, the first that is no error.
     */
    private Type branchType(JCConditional c, JCExpression branch, JCExpression other) {
        for (Type type : new Type[] {branch.type, c.type, other.type}) {
            if (type != null && !type.isErroneous()) {
                return type;
            }
        }
        return syms.objectType;
    }

    /**
     * Makes class {@code c}, which the JVM cannot load for the missing class that {@code missing}
     * names, throw in each of its methods and constructors, as each use of the class throws in
     * Java. Its initializers are left out, since they never run, and so are its missing
     * supertypes.
     */
    private void unloadable(JCClassDecl c, Missing missing, Map<Tree, String> pieces) {
        String binary = missing.binary();
        // A local or anonymous class is part of the piece of code that declares it.
        TreePath at = missing.path();
        while (at != null && at.getLeaf() != c) {
            at = at.getParentPath();
        }
        boolean local = at != null && !member(at);
        if (local) {
            for (TreePath outer = at; outer.getParentPath() != null; outer = outer.getParentPath()) {
                if (outer.getParentPath().getLeaf() instanceof JCClassDecl && member(outer.getParentPath())) {
                    pieces.putIfAbsent(outer.getLeaf(), binary);
                    break;
                }
            }
        }
        for (JCTree member : c.defs) {
            int pos = TreeInfo.getStartPos(member);
            switch (member) {
                case JCMethodDecl method when method.body != null -> {
                    method.body.stats = com.sun.tools.javac.util.List.of(throwing(binary, pos));
                    if (!local) {
                        pieces.putIfAbsent(member, binary);
                    }
                }
                case JCBlock block -> {
                    block.stats = com.sun.tools.javac.util.List.nil();
                    if (!local) {
                        pieces.putIfAbsent(member, binary);
                    }
                }
                case JCVariableDecl variable when variable.init != null -> {
                    variable.init = null;
                    if (!local) {
                        pieces.putIfAbsent(member, binary);
                    }
                }
                default -> {}
            }
        }
        if (c.extending != null && c.extending.type != null && c.extending.type.isErroneous()) {
            c.extending = null;
        }
        c.implementing = c.implementing.stream().filter(i -> i.type == null || !i.type.isErroneous())
                .collect(com.sun.tools.javac.util.List.collector());
        if (c.sym != null && c.sym.type instanceof Type.ClassType type) {
            if (type.supertype_field != null && type.supertype_field.isErroneous()) {
                type.supertype_field = syms.objectType;
            }
            if (type.interfaces_field != null) {
                type.interfaces_field = type.interfaces_field.stream().filter(i -> !i.isErroneous())
                        .collect(com.sun.tools.javac.util.List.collector());
            }
            type.all_interfaces_field = null;
        }
    }

    /** {@code throw new NoClassDefFoundError("binary/name")}, at {@code pos}. */
    private JCThrow throwing(String binary, int pos) {
        if (error == null) {
            error = (ClassSymbol) elements.getTypeElement("java.lang.NoClassDefFoundError");
            for (Symbol s : error.members().getSymbolsByName(names.init)) {
                if (s instanceof MethodSymbol m && m.params().size() == 1
                        && m.params().head.type.tsym == syms.stringType.tsym) {
                    errorOfMessage = m;
                }
            }
        }
        make.at(pos);
        JCNewClass created = make.NewClass(null, com.sun.tools.javac.util.List.nil(), make.Ident(error),
                com.sun.tools.javac.util.List.of(make.Literal(binary)), null);
        created.constructor = errorOfMessage;
        created.constructorType = errorOfMessage.type;
        created.type = error.type;
        return make.Throw(created);
    }

    /**
     * An expression of {@code type} that throws {@code NoClassDefFoundError}, at {@code pos}:
     * {@code switch (0) { default -> throw new NoClassDefFoundError("binary/name"); }}.
     */
    private JCSwitchExpression throwingValue(String binary, int pos, Type type) {
        JCThrow throwing = throwing(binary, pos);
        make.at(pos);
        JCCase only = make.Case(CaseTree.CaseKind.RULE, com.sun.tools.javac.util.List.of(make.DefaultCaseLabel()), null,
                com.sun.tools.javac.util.List.of(throwing), throwing);
        only.completesNormally = false;
        JCSwitchExpression value = make.SwitchExpression(make.Literal(0), com.sun.tools.javac.util.List.of(only));
        value.type = type;
        value.hasUnconditionalPattern = true;
        return value;
    }
}
