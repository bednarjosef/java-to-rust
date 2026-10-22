package java.lang;

import java.util.LinkedHashSet;

/**
 * The translator's patch of the JDK's {@code ModuleLayer} (see docs/jdk-overlays.md and
 * docs/reflection-and-plugins.md).
 *
 * <p>A translated program is closed: its one layer is the boot layer, whose modules are the
 * translation's named modules ({@link Module}), over the empty layer. A layer of more modules
 * would load code that is not in the translation, so defining one throws, saying so, and so does
 * {@link #configuration()}, the module graph that such a layer is resolved against (javac asks
 * for it only to load annotation processors from a module path).
 */
class ModuleLayer {
    @Replace
    private static final ModuleLayer EMPTY_LAYER = new ModuleLayer(List.of(), Set.of());

    @Add
    private static ModuleLayer boot;

    @Replace
    private final Set<Module> modules;

    @Add
    private ModuleLayer(List<ModuleLayer> parents, Set<Module> modules) {
        this.parents = parents;
        this.modules = modules;
    }

    @Add
    private static UnsupportedOperationException notInTranslation(String what) {
        return new UnsupportedOperationException(what + ": code that is not in the translation cannot run");
    }

    /** Java's: what defines a layer, which the translation does not make. */
    @Patch
    class Controller {
        @Add
        private Controller() {
        }

        @Replace
        public ModuleLayer layer() {
            throw notInTranslation("a module layer's controller");
        }

        @Replace
        public Controller addReads(Module source, Module target) {
            throw notInTranslation("a module layer's controller");
        }

        @Replace
        public Controller enableNativeAccess(Module target) {
            throw notInTranslation("a module layer's controller");
        }

        @Replace
        public Controller addExports(Module source, String pn, Module target) {
            throw notInTranslation("a module layer's controller");
        }

        @Replace
        public Controller addOpens(Module source, String pn, Module target) {
            throw notInTranslation("a module layer's controller");
        }

        @Remove
        private final ModuleLayer layer;

        @Remove
        Controller(ModuleLayer layer);

        @Remove
        private void ensureInLayer(Module source);
    }

    @Replace
    public ModuleLayer defineModulesWithOneLoader(Configuration cf, ClassLoader parentLoader) {
        throw notInTranslation("defining a module layer");
    }

    @Replace
    public ModuleLayer defineModulesWithManyLoaders(Configuration cf, ClassLoader parentLoader) {
        throw notInTranslation("defining a module layer");
    }

    @Replace
    public ModuleLayer defineModules(Configuration cf, Function<String, ClassLoader> clf) {
        throw notInTranslation("defining a module layer");
    }

    @Replace
    public static Controller defineModulesWithOneLoader(Configuration cf, List<ModuleLayer> parentLayers,
            ClassLoader parentLoader) {
        throw notInTranslation("defining a module layer");
    }

    @Replace
    public static Controller defineModulesWithManyLoaders(Configuration cf, List<ModuleLayer> parentLayers,
            ClassLoader parentLoader) {
        throw notInTranslation("defining a module layer");
    }

    @Replace
    public static Controller defineModules(Configuration cf, List<ModuleLayer> parentLayers,
            Function<String, ClassLoader> clf) {
        throw notInTranslation("defining a module layer");
    }

    @Replace
    public Configuration configuration() {
        throw notInTranslation("the configuration of a module layer, which resolves modules,");
    }

    @Replace
    Stream<ModuleLayer> layers() {
        List<ModuleLayer> all = new ArrayList<>();
        all.add(this);
        for (int i = 0; i < all.size(); i++) {
            for (ModuleLayer p : all.get(i).parents) {
                if (!all.contains(p)) {
                    all.add(p);
                }
            }
        }
        return all.stream();
    }

    @Replace
    public Set<Module> modules() {
        return modules;
    }

    @Replace
    public Optional<Module> findModule(String name) {
        Objects.requireNonNull(name);
        for (Module m : modules) {
            if (name.equals(m.getName())) {
                return Optional.of(m);
            }
        }
        for (ModuleLayer p : parents) {
            Optional<Module> found = p.findModule(name);
            if (found.isPresent()) {
                return found;
            }
        }
        return Optional.empty();
    }

    @Replace
    boolean addEnableNativeAccess(String name) {
        throw notInTranslation("native access");
    }

    @Replace
    public ClassLoader findLoader(String name) {
        Optional<Module> module = findModule(name);
        if (module.isEmpty()) {
            throw new IllegalArgumentException("Module " + name + " not known to this layer");
        }
        return module.get().getClassLoader();
    }

    @Replace
    public String toString() {
        return modules.stream().map(Module::getName).collect(Collectors.joining(", "));
    }

    @Replace
    public static synchronized ModuleLayer boot() {
        if (boot == null) {
            boot = new ModuleLayer(List.of(EMPTY_LAYER),
                    Collections.unmodifiableSet(new LinkedHashSet<>(Module.named().values())));
        }
        return boot;
    }

    @Replace
    ServicesCatalog getServicesCatalog() {
        throw notInTranslation("the services catalog of a module layer");
    }

    @Replace
    void bindToLoader(ClassLoader loader) {
        throw notInTranslation("binding a module layer to a class loader");
    }

    @Replace
    static Stream<ModuleLayer> layers(ClassLoader loader) {
        throw notInTranslation("the module layers of a class loader");
    }
    // Java's reads the empty layer from the CDS archive or makes it; here it is made where it is
    // declared.
    static {}

    @Remove
    private final Configuration cf;

    @Remove
    private final Map<String, Module> nameToModule;

    @Remove
    private ModuleLayer(Configuration cf, List<ModuleLayer> parents, Function<String, ClassLoader> clf);

    @Remove
    private static void checkConfiguration(Configuration cf, List<ModuleLayer> parentLayers);

    @Remove
    private static void checkForDuplicatePkgs(Configuration cf, Function<String, ClassLoader> clf);

    @Remove
    private static LayerInstantiationException fail(String fmt, Object ... args);

    @Remove
    private volatile List<ModuleLayer> allLayers;

    @Remove
    private volatile ServicesCatalog servicesCatalog;

    @Remove
    private static final ClassLoaderValue<List<ModuleLayer>> CLV;
}
