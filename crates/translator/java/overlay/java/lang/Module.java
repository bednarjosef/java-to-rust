package java.lang;

import java.io.IOException;
import java.io.InputStream;
import java.lang.annotation.Annotation;
import java.lang.module.Configuration;
import java.lang.module.ModuleDescriptor;
import java.net.URI;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.function.Function;

/**
 * The translator's {@code Module}, which replaces the JDK's in a translation with the JDK's
 * sources (see docs/reflection-and-plugins.md).
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
public final class Module {
    /** The modules of the JDK that the JVM's boot loader defines (Java 26). */
    private static final String[] BOOT_MODULES = {
        "java.base", "java.datatransfer", "java.desktop", "java.instrument", "java.logging",
        "java.management", "java.management.rmi", "java.naming", "java.prefs", "java.rmi",
        "java.security.sasl", "java.xml", "jdk.jfr", "jdk.management", "jdk.management.agent",
        "jdk.management.jfr", "jdk.naming.rmi", "jdk.net", "jdk.nio.mapmode", "jdk.sctp",
        "jdk.unsupported",
    };

    /** The modules of the JDK that the JVM's platform loader defines (Java 26). */
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
    private static LinkedHashMap<String, Module> named;
    /** The unnamed modules of the boot loader and of the class loaders asked so far. */
    private static Module bootUnnamed;
    private static final List<Module> UNNAMED = new ArrayList<>();
    /** What a module adds a read, an export or an open to for all unnamed modules. */
    private static final Module ALL_UNNAMED_MODULE = new Module((ClassLoader) null);

    private final String name;
    private final ClassLoader loader;
    private final boolean open;
    private final List<String> requires = new ArrayList<>();
    private final List<String> transitive = new ArrayList<>();
    private final Set<String> packages = new HashSet<>();
    /** The packages it exports or opens, each to the names of the modules it does so to (empty: all). */
    private final Map<String, Set<String>> exports = new HashMap<>();
    private final Map<String, Set<String>> opens = new HashMap<>();
    private final Set<String> uses = new HashSet<>();
    /**
     * What `addReads`, `addExports`, `addOpens` and `addUses` added; null in the modules added to
     * is everyone, and `ALL_UNNAMED_MODULE` every unnamed module.
     */
    private final Set<Module> addedReads = new HashSet<>();
    private final Map<String, Set<Module>> addedExports = new HashMap<>();
    private final Map<String, Set<Module>> addedOpens = new HashMap<>();
    private final Set<String> addedUses = new HashSet<>();

    // A named module of the translation, from its declaration (`declares0`).
    private Module(String name, String[] declared) {
        this.name = name;
        this.loader = loaderOf(name);
        boolean open = false;
        for (String d : declared) {
            String[] w = d.split(" ");
            switch (w[0]) {
                case "open" -> open = true;
                case "requires" -> requires.add(w[1]);
                case "transitive" -> {
                    requires.add(w[1]);
                    transitive.add(w[1]);
                }
                case "package" -> packages.add(w.length > 1 ? w[1] : "");
                case "exports", "opens" -> {
                    Set<String> to = new HashSet<>();
                    for (int i = 2; i < w.length; i++) {
                        to.add(w[i]);
                    }
                    (w[0].equals("exports") ? exports : opens).put(w[1], to);
                }
                case "uses" -> uses.add(w[1]);
                default -> throw new InternalError("a module declares " + d);
            }
        }
        this.open = open;
    }

    // Java's: a module of a layer that defines modules, which the translation does not do.
    Module(ModuleLayer layer, ClassLoader loader, ModuleDescriptor descriptor, URI uri) {
        throw notInTranslation("defining the module " + descriptor.name());
    }

    // The unnamed module of the class loader `loader`.
    Module(ClassLoader loader) {
        this.name = null;
        this.loader = loader;
        this.open = true;
    }

    // Java's: a named module of a class loader without a layer, which the translation does not make.
    Module(ClassLoader loader, ModuleDescriptor descriptor) {
        throw notInTranslation("defining the module " + descriptor.name());
    }

    private static UnsupportedOperationException notInTranslation(String what) {
        return new UnsupportedOperationException(what + ": code that is not in the translation cannot run");
    }

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
    static Module of(Class<?> c) {
        String module = moduleOf0(c.getPackageName());
        if (module != null) {
            return named().get(module);
        }
        // The program's classes outside its named modules, which the system class loader loads.
        return unnamedOf(ClassLoader.getSystemClassLoader());
    }

    /** The unnamed module of the class loader {@code loader} (null: the boot loader's). */
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
    static Class<?> within(Class<?> c, Module module) {
        Objects.requireNonNull(module);
        if (c == null) {
            return null;
        }
        return of(c) == module ? c : null;
    }

    public boolean isNamed() {
        return name != null;
    }

    public String getName() {
        return name;
    }

    public ClassLoader getClassLoader() {
        return loader;
    }

    public ModuleDescriptor getDescriptor() {
        if (!isNamed()) {
            return null;
        }
        throw notInTranslation("the descriptor of module " + name);
    }

    public ModuleLayer getLayer() {
        return isNamed() ? ModuleLayer.boot() : null;
    }

    Module implAddEnableNativeAccess() {
        throw notInTranslation("native access");
    }

    public boolean isNativeAccessEnabled() {
        throw notInTranslation("native access");
    }

    void ensureNativeAccess(Class<?> owner, String methodName, Class<?> currentClass, boolean jni) {
        throw notInTranslation("native access");
    }

    static void addEnableNativeAccessToAllUnnamed() {
        throw notInTranslation("native access");
    }

    static void addEnableFinalMutationToAllUnnamed() {
        throw notInTranslation("final field mutation");
    }

    boolean tryEnableFinalMutation() {
        throw notInTranslation("final field mutation");
    }

    boolean isFinalMutationEnabled() {
        throw notInTranslation("final field mutation");
    }

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

    public Module addReads(Module other) {
        Objects.requireNonNull(other);
        implAddReads(other);
        return this;
    }

    void implAddReads(Module other) {
        if (isNamed()) {
            addedReads.add(other);
        }
    }

    void implAddReadsAllUnnamed() {
        if (isNamed()) {
            addedReads.add(ALL_UNNAMED_MODULE);
        }
    }

    void implAddReadsNoSync(Module other) {
        implAddReads(other);
    }

    public boolean isExported(String pn, Module other) {
        Objects.requireNonNull(pn);
        Objects.requireNonNull(other);
        return exportedOrOpen(pn, other, false);
    }

    public boolean isOpen(String pn, Module other) {
        Objects.requireNonNull(pn);
        Objects.requireNonNull(other);
        return exportedOrOpen(pn, other, true);
    }

    public boolean isExported(String pn) {
        Objects.requireNonNull(pn);
        return exportedOrOpen(pn, null, false);
    }

    public boolean isOpen(String pn) {
        Objects.requireNonNull(pn);
        return exportedOrOpen(pn, null, true);
    }

    // Java's `implIsExportedOrOpen`, `other` null being everyone.
    private boolean exportedOrOpen(String pn, Module other, boolean open) {
        return staticallyExportedOrOpen(pn, other, open) || reflectivelyExportedOrOpen(pn, other, open);
    }

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

    private static boolean allows(Set<String> targets, Module other) {
        return targets != null && (targets.isEmpty() || other != null && other.isNamed() && targets.contains(other.name));
    }

    // An open package is exported too.
    private boolean reflectivelyExportedOrOpen(String pn, Module other, boolean open) {
        return added(addedOpens.get(pn), other) || !open && added(addedExports.get(pn), other);
    }

    private static boolean added(Set<Module> to, Module other) {
        return to != null
                && (to.contains(null)
                        || other != null && (to.contains(other) || !other.isNamed() && to.contains(ALL_UNNAMED_MODULE)));
    }

    boolean isStaticallyExported(String pn, Module other) {
        return staticallyExportedOrOpen(pn, other, false);
    }

    boolean isStaticallyOpened(String pn, Module other) {
        return staticallyExportedOrOpen(pn, other, true);
    }

    boolean isReflectivelyExported(String pn, Module other) {
        return reflectivelyExportedOrOpen(pn, other, false);
    }

    boolean isReflectivelyOpened(String pn, Module other) {
        return reflectivelyExportedOrOpen(pn, other, true);
    }

    public Module addExports(String pn, Module other) {
        if (pn == null) {
            throw new IllegalArgumentException("package is null");
        }
        Objects.requireNonNull(other);
        implAddExports(pn, other);
        return this;
    }

    public Module addOpens(String pn, Module other) {
        if (pn == null) {
            throw new IllegalArgumentException("package is null");
        }
        Objects.requireNonNull(other);
        implAddOpens(pn, other);
        return this;
    }

    // `other` null: to everyone. As in Java, a module that already exports or opens the
    // package so is not changed, and one that does not have it throws.
    private void add(String pn, Module other, boolean open) {
        if (!isNamed() || this.open || exportedOrOpen(pn, other, open)) {
            return;
        }
        if (!packages.contains(pn)) {
            throw new IllegalArgumentException("package " + pn + " not in contents");
        }
        (open ? addedOpens : addedExports).computeIfAbsent(pn, k -> new HashSet<>()).add(other);
    }

    void implAddExports(String pn) {
        add(pn, null, false);
    }

    void implAddExports(String pn, Module other) {
        add(pn, other, false);
    }

    void implAddExportsToAllUnnamed(String pn) {
        add(pn, ALL_UNNAMED_MODULE, false);
    }

    void implAddExportsNoSync(String pn) {
        add(pn.replace('/', '.'), null, false);
    }

    void implAddExportsNoSync(String pn, Module other) {
        add(pn.replace('/', '.'), other, false);
    }

    void implAddOpens(String pn) {
        add(pn, null, true);
    }

    void implAddOpens(String pn, Module other) {
        add(pn, other, true);
    }

    void implAddOpensToAllUnnamed(String pn) {
        add(pn, ALL_UNNAMED_MODULE, true);
    }

    public Module addUses(Class<?> service) {
        Objects.requireNonNull(service);
        implAddUses(service);
        return this;
    }

    void implAddUses(Class<?> service) {
        if (isNamed()) {
            addedUses.add(service.getName().replace('$', '.'));
        }
    }

    public boolean canUse(Class<?> service) {
        Objects.requireNonNull(service);
        if (!isNamed()) {
            return true;
        }
        String n = service.getName().replace('$', '.');
        return uses.contains(n) || addedUses.contains(n);
    }

    public Set<String> getPackages() {
        if (!isNamed()) {
            throw notInTranslation("the packages of " + this + ", which are those its loader defined so far,");
        }
        return Set.copyOf(packages);
    }

    static Map<String, Module> defineModules(Configuration cf, Function<String, ClassLoader> clf, ModuleLayer layer) {
        throw notInTranslation("defining modules");
    }

    public <T extends Annotation> T getAnnotation(Class<T> annotationClass) {
        throw notInTranslation("the annotations of " + this);
    }

    public Annotation[] getAnnotations() {
        throw notInTranslation("the annotations of " + this);
    }

    public Annotation[] getDeclaredAnnotations() {
        throw notInTranslation("the annotations of " + this);
    }

    // `AnnotatedElement`'s, which the translation does not implement, as it has no annotations.
    public boolean isAnnotationPresent(Class<? extends Annotation> annotationClass) {
        throw notInTranslation("the annotations of " + this);
    }

    public <T extends Annotation> T[] getAnnotationsByType(Class<T> annotationClass) {
        throw notInTranslation("the annotations of " + this);
    }

    public <T extends Annotation> T getDeclaredAnnotation(Class<T> annotationClass) {
        throw notInTranslation("the annotations of " + this);
    }

    public <T extends Annotation> T[] getDeclaredAnnotationsByType(Class<T> annotationClass) {
        throw notInTranslation("the annotations of " + this);
    }

    public InputStream getResourceAsStream(String name) throws IOException {
        if (name.startsWith("/")) {
            name = name.substring(1);
        }
        if (holds0(isNamed() ? this.name : "", name)) {
            throw notInTranslation("the resource " + name + " of " + this);
        }
        return null;
    }

    public String toString() {
        return isNamed() ? "module " + name : "unnamed module @" + Integer.toHexString(System.identityHashCode(this));
    }

    /** The module of the boot loader whose root holds the file {@code name}, or null. */
    static String bootHolder(String name) {
        for (String m : BOOT_MODULES) {
            if (holds0(m, name)) {
                return m;
            }
        }
        return null;
    }

    /** The names of the translation's named modules, in the order of their roots. */
    private static native String[] modules0();

    /** The name of the named module with the package {@code pn}, or null. */
    private static native String moduleOf0(String pn);

    /**
     * What the named module {@code name} declares and holds, each a line of words: {@code open},
     * {@code requires m}, {@code transitive m}, {@code exports p [m..]}, {@code opens p [m..]},
     * {@code uses s} and {@code package p}.
     */
    private static native String[] declares0(String name);

    /** Whether the root of the module {@code module} ("" the unnamed one) holds the file {@code name}. */
    private static native boolean holds0(String module, String name);
}
