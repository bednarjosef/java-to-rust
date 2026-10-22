package java.lang;

import java.util.ArrayList;
import java.util.LinkedHashMap;

/**
 * The translator's patch of the JDK's {@code Module} (see docs/jdk-overlays.md and
 * docs/reflection-and-plugins.md).
 *
 * <p>A translated program is closed: its modules are those of its roots, each root with a
 * {@code module-info.java} a named module, and the roots without one the unnamed module of the
 * system class loader. A class is in the module whose root has its package, which the translator
 * tells when it translates (the natives {@code modules0}, {@code moduleOf0}, {@code declares0}
 * and {@code holds0}, see {@code lower/modules.rs}); the translator's own classes are in
 * {@code java.base}. A named module answers from its declaration, as the JVM's boot layer does:
 * what it reads (what it requires, transitively through {@code requires transitive}, and
 * {@code java.base}), exports, opens and uses, and its class loader, which is the JVM's for the
 * JDK's modules ({@code BOOT_MODULES}, {@code PLATFORM_MODULES}) and the system class loader for
 * any other. {@code addReads}, {@code addExports}, {@code addOpens} and {@code addUses} add to
 * that as in Java, but do not check that their caller is in the module, so they never throw
 * {@code IllegalCallerException}.
 *
 * <p>What would need code or data that is not in the translation throws
 * {@code UnsupportedOperationException} when it runs, saying so: a module's descriptor and
 * annotations, defining modules, native access and final field mutation. A resource that a
 * root holds is not in the translation either, so {@link #getResourceAsStream} of one throws, and
 * of any other name gives null, as Java does.
 */
// Not an `AnnotatedElement`: a module's annotations are not in the translation.
@Edit(find = "implements AnnotatedElement", replace = "")
class Module {
    /** The modules of the JDK that the JVM's boot loader defines (Java 26). */
    @Add
    private static final String[] BOOT_MODULES = {
        "java.base", "java.datatransfer", "java.desktop", "java.instrument", "java.logging",
        "java.management", "java.management.rmi", "java.naming", "java.prefs", "java.rmi",
        "java.security.sasl", "java.xml", "jdk.jfr", "jdk.management", "jdk.management.agent",
        "jdk.management.jfr", "jdk.naming.rmi", "jdk.net", "jdk.nio.mapmode", "jdk.sctp",
        "jdk.unsupported",
    };

    /** The modules of the JDK that the JVM's platform loader defines (Java 26). */
    @Add
    private static final String[] PLATFORM_MODULES = {
        "java.compiler", "java.net.http", "java.scripting", "java.security.jgss", "java.smartcardio",
        "java.sql", "java.sql.rowset", "java.transaction.xa", "java.xml.crypto", "jdk.accessibility",
        "jdk.charsets", "jdk.crypto.cryptoki", "jdk.dynalink", "jdk.httpserver", "jdk.localedata",
        "jdk.naming.dns", "jdk.security.auth", "jdk.security.jgss", "jdk.xml.dom", "jdk.zipfs",
    };

    /**
     * The translation's named modules by name, made when first asked for. (A `LinkedHashMap`, so
     * that its calls reach only that class's methods, not every map's.)
     */
    @Add
    private static LinkedHashMap<String, Module> named;

    /** The unnamed modules of the boot loader and of the class loaders asked so far. */
    @Add
    private static Module bootUnnamed;

    @Add
    private static final List<Module> UNNAMED = new ArrayList<>();

    /** What a module adds a read, an export or an open to for all unnamed modules. */
    @Replace
    private static final Module ALL_UNNAMED_MODULE = new Module((ClassLoader) null);

    @Add
    private final boolean open;

    @Add
    private final List<String> requires = new ArrayList<>();

    @Add
    private final List<String> transitive = new ArrayList<>();

    @Add
    private final Set<String> packages = new HashSet<>();

    /** The packages it exports or opens, each to the names of the modules it does so to (empty: all). */
    @Add
    private final Map<String, Set<String>> exports = new HashMap<>();

    @Add
    private final Map<String, Set<String>> opens = new HashMap<>();

    @Add
    private final Set<String> uses = new HashSet<>();

    /**
     * What `addReads`, `addExports`, `addOpens` and `addUses` added; null in the modules added to
     * is everyone, and `ALL_UNNAMED_MODULE` every unnamed module.
     */
    @Add
    private final Set<Module> addedReads = new HashSet<>();

    @Add
    private final Map<String, Set<Module>> addedExports = new HashMap<>();

    @Add
    private final Map<String, Set<Module>> addedOpens = new HashMap<>();

    @Add
    private final Set<String> addedUses = new HashSet<>();

    // A named module of the translation, from its declaration (`declares0`).
    @Add
    private Module(String name, String[] declared) {
        this.name = name;
        this.loader = loaderOf(name);
        boolean open = false;
        for (String d : declared) {
            String[] w = d.split(" ");
            // (A chain of `equals` on the words, not a `switch`: the switch's temporary has a
            // slot that only javac's code generator knows, which a `NullPointerException`'s
            // message would name.)
            String what = w[0];
            if ("open".equals(what)) {
                open = true;
            } else if ("requires".equals(what)) {
                requires.add(w[1]);
            } else if ("transitive".equals(what)) {
                requires.add(w[1]);
                transitive.add(w[1]);
            } else if ("package".equals(what)) {
                packages.add(w.length > 1 ? w[1] : "");
            } else if ("exports".equals(what) || "opens".equals(what)) {
                Set<String> to = new HashSet<>();
                for (int i = 2; i < w.length; i++) {
                    to.add(w[i]);
                }
                ("exports".equals(what) ? exports : opens).put(w[1], to);
            } else if ("uses".equals(what)) {
                uses.add(w[1]);
            } else {
                throw new InternalError("a module declares " + d);
            }
        }
        this.open = open;
    }

    // Java's: a module of a layer that defines modules, which the translation does not do.
    @Replace
    Module(ModuleLayer layer, ClassLoader loader, ModuleDescriptor descriptor, URI uri) {
        throw notInTranslation("defining the module " + descriptor.name());
    }

    // The unnamed module of the class loader `loader`.
    @Replace
    Module(ClassLoader loader) {
        this.name = null;
        this.loader = loader;
        this.open = true;
    }

    // Java's: a named module of a class loader without a layer, which the translation does not make.
    @Replace
    Module(ClassLoader loader, ModuleDescriptor descriptor) {
        throw notInTranslation("defining the module " + descriptor.name());
    }

    @Add
    private static UnsupportedOperationException notInTranslation(String what) {
        return new UnsupportedOperationException(what + ": code that is not in the translation cannot run");
    }

    @Add
    private static ClassLoader loaderOf(String name) {
        for (String m : BOOT_MODULES) {
            if (m.equals(name)) {
                return null;
            }
        }
        for (String m : PLATFORM_MODULES) {
            if (m.equals(name)) {
                return ClassLoader.getPlatformClassLoader();
            }
        }
        return ClassLoader.getSystemClassLoader();
    }

    // The translation's named modules, in the order of their roots.
    @Add
    static synchronized LinkedHashMap<String, Module> named() {
        if (named == null) {
            LinkedHashMap<String, Module> all = new LinkedHashMap<>();
            for (String n : modules0()) {
                all.put(n, new Module(n, declares0(n)));
            }
            named = all;
        }
        return named;
    }

    /** The module of the class {@code c}: {@code c.getModule()}. */
    @Add
    static Module of(Class<?> c) {
        String module = moduleOf0(c.getPackageName());
        if (module != null) {
            return named().get(module);
        }
        // The program's classes outside its named modules, which the system class loader loads.
        return unnamedOf(ClassLoader.getSystemClassLoader());
    }

    /** The unnamed module of the class loader {@code loader} (null: the boot loader's). */
    @Add
    static synchronized Module unnamedOf(ClassLoader loader) {
        if (loader == null) {
            if (bootUnnamed == null) {
                bootUnnamed = new Module((ClassLoader) null);
            }
            return bootUnnamed;
        }
        for (Module m : UNNAMED) {
            if (m.loader == loader) {
                return m;
            }
        }
        Module m = new Module(loader);
        UNNAMED.add(m);
        return m;
    }

    /** {@code Class.forName(module, name)}: the class {@code c} that a name found, if it is in {@code module}. */
    @Add
    static Class<?> within(Class<?> c, Module module) {
        Objects.requireNonNull(module);
        if (c == null) {
            return null;
        }
        return of(c) == module ? c : null;
    }

    @Replace
    public ModuleDescriptor getDescriptor() {
        if (!isNamed()) {
            return null;
        }
        throw notInTranslation("the descriptor of module " + name);
    }

    @Replace
    public ModuleLayer getLayer() {
        return isNamed() ? ModuleLayer.boot() : null;
    }

    @Replace
    Module implAddEnableNativeAccess() {
        throw notInTranslation("native access");
    }

    @Replace
    public boolean isNativeAccessEnabled() {
        throw notInTranslation("native access");
    }

    @Replace
    void ensureNativeAccess(Class<?> owner, String methodName, Class<?> currentClass, boolean jni) {
        throw notInTranslation("native access");
    }

    @Replace
    static void addEnableNativeAccessToAllUnnamed() {
        throw notInTranslation("native access");
    }

    @Replace
    static void addEnableFinalMutationToAllUnnamed() {
        throw notInTranslation("final field mutation");
    }

    @Replace
    boolean tryEnableFinalMutation() {
        throw notInTranslation("final field mutation");
    }

    @Replace
    boolean isFinalMutationEnabled() {
        throw notInTranslation("final field mutation");
    }

    @Replace
    public boolean canRead(Module other) {
        Objects.requireNonNull(other);
        if (!isNamed() || other == this) {
            return true;
        }
        if (addedReads.contains(other) || !other.isNamed() && addedReads.contains(ALL_UNNAMED_MODULE)) {
            return true;
        }
        return other.isNamed() && reads(other.name);
    }

    // Whether this named module reads the named module `other` from its declaration: what it
    // requires, what those require transitively, and `java.base`.
    @Add
    private boolean reads(String other) {
        if (other.equals("java.base")) {
            return true;
        }
        List<String> work = new ArrayList<>(requires);
        Set<String> seen = new HashSet<>();
        for (int i = 0; i < work.size(); i++) {
            String m = work.get(i);
            if (m.equals(other)) {
                return true;
            }
            Module required = named().get(m);
            if (seen.add(m) && required != null) {
                work.addAll(required.transitive);
            }
        }
        return false;
    }

    @Replace
    public Module addReads(Module other) {
        Objects.requireNonNull(other);
        implAddReads(other);
        return this;
    }

    @Replace
    void implAddReads(Module other) {
        if (isNamed()) {
            addedReads.add(other);
        }
    }

    @Replace
    void implAddReadsAllUnnamed() {
        if (isNamed()) {
            addedReads.add(ALL_UNNAMED_MODULE);
        }
    }

    @Replace
    void implAddReadsNoSync(Module other) {
        implAddReads(other);
    }

    @Replace
    public boolean isExported(String pn, Module other) {
        Objects.requireNonNull(pn);
        Objects.requireNonNull(other);
        return exportedOrOpen(pn, other, false);
    }

    @Replace
    public boolean isOpen(String pn, Module other) {
        Objects.requireNonNull(pn);
        Objects.requireNonNull(other);
        return exportedOrOpen(pn, other, true);
    }

    @Replace
    public boolean isExported(String pn) {
        Objects.requireNonNull(pn);
        return exportedOrOpen(pn, null, false);
    }

    @Replace
    public boolean isOpen(String pn) {
        Objects.requireNonNull(pn);
        return exportedOrOpen(pn, null, true);
    }

    // Java's `implIsExportedOrOpen`, `other` null being everyone.
    @Add
    private boolean exportedOrOpen(String pn, Module other, boolean open) {
        return staticallyExportedOrOpen(pn, other, open) || reflectivelyExportedOrOpen(pn, other, open);
    }

    @Add
    private boolean staticallyExportedOrOpen(String pn, Module other, boolean open) {
        if (!isNamed()) {
            return true;
        }
        if (other == this && packages.contains(pn)) {
            return true;
        }
        if (this.open) {
            return packages.contains(pn);
        }
        return allows(opens.get(pn), other) || !open && allows(exports.get(pn), other);
    }

    @Add
    private static boolean allows(Set<String> targets, Module other) {
        return targets != null && (targets.isEmpty() || other != null && other.isNamed() && targets.contains(other.name));
    }

    // An open package is exported too.
    @Add
    private boolean reflectivelyExportedOrOpen(String pn, Module other, boolean open) {
        return added(addedOpens.get(pn), other) || !open && added(addedExports.get(pn), other);
    }

    @Add
    private static boolean added(Set<Module> to, Module other) {
        return to != null
                && (to.contains(null)
                        || other != null && (to.contains(other) || !other.isNamed() && to.contains(ALL_UNNAMED_MODULE)));
    }

    @Replace
    boolean isStaticallyExported(String pn, Module other) {
        return staticallyExportedOrOpen(pn, other, false);
    }

    @Replace
    boolean isStaticallyOpened(String pn, Module other) {
        return staticallyExportedOrOpen(pn, other, true);
    }

    @Replace
    boolean isReflectivelyExported(String pn, Module other) {
        return reflectivelyExportedOrOpen(pn, other, false);
    }

    @Replace
    boolean isReflectivelyOpened(String pn, Module other) {
        return reflectivelyExportedOrOpen(pn, other, true);
    }

    // No check of the caller's module (see the class's comment).
    @Edit(from = "Module caller = getCallerModule(", to = "}", replace = "")
    public Module addExports(String pn, Module other);

    // No check of the caller's module (see the class's comment).
    @Edit(from = "Module caller = getCallerModule(", to = "is not open to \" + caller);", replace = "")
    public Module addOpens(String pn, Module other);

    // `other` null: to everyone. As in Java, a module that already exports or opens the
    // package so is not changed, and one that does not have it throws.
    @Add
    private void add(String pn, Module other, boolean open) {
        if (!isNamed() || this.open || exportedOrOpen(pn, other, open)) {
            return;
        }
        if (!packages.contains(pn)) {
            throw new IllegalArgumentException("package " + pn + " not in contents");
        }
        (open ? addedOpens : addedExports).computeIfAbsent(pn, k -> new HashSet<>()).add(other);
    }

    @Replace
    void implAddExports(String pn) {
        add(pn, null, false);
    }

    @Replace
    void implAddExports(String pn, Module other) {
        add(pn, other, false);
    }

    @Replace
    void implAddExportsToAllUnnamed(String pn) {
        add(pn, ALL_UNNAMED_MODULE, false);
    }

    @Replace
    void implAddExportsNoSync(String pn) {
        add(pn.replace('/', '.'), null, false);
    }

    @Replace
    void implAddExportsNoSync(String pn, Module other) {
        add(pn.replace('/', '.'), other, false);
    }

    @Replace
    void implAddOpens(String pn) {
        add(pn, null, true);
    }

    @Replace
    void implAddOpens(String pn, Module other) {
        add(pn, other, true);
    }

    @Replace
    void implAddOpensToAllUnnamed(String pn) {
        add(pn, ALL_UNNAMED_MODULE, true);
    }

    @Replace
    public Module addUses(Class<?> service) {
        nonNull(service);
        implAddUses(service);
        return this;
    }

    @Replace
    void implAddUses(Class<?> service) {
        if (isNamed()) {
            addedUses.add(service.getName().replace('$', '.'));
        }
    }

    @Replace
    public boolean canUse(Class<?> service) {
        nonNull(service);
        if (!isNamed()) {
            return true;
        }
        String n = service.getName().replace('$', '.');
        return uses.contains(n) || addedUses.contains(n);
    }

    @Replace
    public Set<String> getPackages() {
        if (!isNamed()) {
            throw notInTranslation("the packages of " + this + ", which are those its loader defined so far,");
        }
        return Set.copyOf(packages);
    }

    @Replace
    static Map<String, Module> defineModules(Configuration cf, Function<String, ClassLoader> clf, ModuleLayer layer) {
        throw notInTranslation("defining modules");
    }

    @Replace
    public <T extends Annotation> T getAnnotation(Class<T> annotationClass) {
        throw notInTranslation("the annotations of " + this);
    }

    @Replace
    public Annotation[] getAnnotations() {
        throw notInTranslation("the annotations of " + this);
    }

    @Replace
    public Annotation[] getDeclaredAnnotations() {
        throw notInTranslation("the annotations of " + this);
    }

    // `AnnotatedElement`'s, which the translation does not implement, as it has no annotations.
    @Add
    public boolean isAnnotationPresent(Class<? extends Annotation> annotationClass) {
        throw notInTranslation("the annotations of " + this);
    }

    @Add
    public <T extends Annotation> T[] getAnnotationsByType(Class<T> annotationClass) {
        throw notInTranslation("the annotations of " + this);
    }

    @Add
    public <T extends Annotation> T getDeclaredAnnotation(Class<T> annotationClass) {
        throw notInTranslation("the annotations of " + this);
    }

    @Add
    public <T extends Annotation> T[] getDeclaredAnnotationsByType(Class<T> annotationClass) {
        throw notInTranslation("the annotations of " + this);
    }

    @Replace
    public InputStream getResourceAsStream(String name) throws IOException {
        if (name.startsWith("/")) {
            name = name.substring(1);
        }
        if (holds0(isNamed() ? this.name : "", name)) {
            throw notInTranslation("the resource " + name + " of " + this);
        }
        return null;
    }

    @Replace
    public String toString() {
        return isNamed() ? "module " + name : "unnamed module @" + Integer.toHexString(System.identityHashCode(this));
    }

    /** The module of the boot loader whose root holds the file {@code name}, or null. */
    @Add
    static String bootHolder(String name) {
        for (String m : BOOT_MODULES) {
            if (holds0(m, name)) {
                return m;
            }
        }
        return null;
    }

    /** The names of the translation's named modules, in the order of their roots. */
    @Add
    private static native String[] modules0();

    /** The name of the named module with the package {@code pn}, or null. */
    @Add
    private static native String moduleOf0(String pn);

    /**
     * What the named module {@code name} declares and holds, each a line of words: {@code open},
     * {@code requires m}, {@code transitive m}, {@code exports p [m..]}, {@code opens p [m..]},
     * {@code uses s} and {@code package p}.
     */
    @Add
    private static native String[] declares0(String name);

    /** Whether the root of the module {@code module} ("" the unnamed one) holds the file {@code name}. */
    @Add
    private static native boolean holds0(String module, String name);

    // `Objects.requireNonNull` of a class, which would hold it as an `Object`.
    @Add
    private static void nonNull(Class<?> c) {
        if (c == null) {
            throw new NullPointerException();
        }
    }

    @Remove
    private final ModuleLayer layer;

    @Remove
    private final ModuleDescriptor descriptor;

    @Remove
    private boolean enableNativeAccess;

    @Remove
    private boolean enableFinalMutation;

    @Remove
    class EnableNativeAccess {}

    @Remove
    private Module moduleForNativeAccess();

    @Remove
    class EnableFinalMutation {}

    @Remove
    private static final Set<Module> ALL_UNNAMED_MODULE_SET;

    @Remove
    private static final Module EVERYONE_MODULE;

    @Remove
    private static final Set<Module> EVERYONE_SET;

    @Remove
    class ArchivedData {}
    // Java's sets the modules that stand for every module and all unnamed ones from the CDS
    // archive or anew; here they are made where they are declared.
    static {}

    @Remove
    class ReflectionData {}

    @Remove
    private volatile Set<Module> reads;

    @Remove
    private void implAddReads(Module other, boolean syncVM);

    @Remove
    private volatile Map<String, Set<Module>> openPackages;

    @Remove
    private volatile Map<String, Set<Module>> exportedPackages;

    @Remove
    private boolean implIsExportedOrOpen(String pn, Module other, boolean open);

    @Remove
    private boolean isStaticallyExportedOrOpened(String pn, Module other, boolean open);

    @Remove
    private boolean isExplicitlyExportedOrOpened(String pn, Module other, boolean open);

    @Remove
    private boolean allows(Set<Module> targets, Module module);

    @Remove
    private boolean isReflectivelyExportedOrOpened(String pn, Module other, boolean open);

    @Remove
    private void implAddExportsOrOpens(String pn, Module other, boolean open, boolean reflectively, boolean syncVM);

    @Remove
    private static Module findModule(ModuleLayer parent, ResolvedModule resolvedModule);

    @Remove
    private static void initExports(Module m, Map<String, Module> nameToModule);

    @Remove
    private static void initExportsAndOpens(Module m, Map<String, Module> nameToSource, Map<String, Module> nameToModule, List<ModuleLayer> parents);

    @Remove
    private static Module findModule(String target, Map<String, Module> nameToSource, Map<String, Module> nameToModule, List<ModuleLayer> parents);

    @Remove
    private volatile Class<?> moduleInfoClass;

    @Remove
    private Class<?> moduleInfoClass();

    @Remove
    private Class<?> loadModuleInfoClass();

    @Remove
    private Class<?> loadModuleInfoClass(InputStream in) throws IOException;

    @Remove
    private Module getCallerModule(Class<?> caller);

    @Remove
    private static native void defineModule0(Module module, boolean isOpen, String version, String location, Object[] pns);

    @Remove
    private static native void addReads0(Module from, Module to);

    @Remove
    private static native void addExports0(Module from, String pn, Module to);

    @Remove
    private static native void addExportsToAll0(Module from, String pn);

    @Remove
    private static native void addExportsToAllUnnamed0(Module from, String pn);
}
