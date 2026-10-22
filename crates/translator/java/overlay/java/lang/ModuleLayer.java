package java.lang;

import java.lang.module.Configuration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Objects;
import java.util.Optional;
import java.util.Set;
import java.util.function.Function;
import java.util.stream.Collectors;
import java.util.stream.Stream;
import jdk.internal.module.ServicesCatalog;

/**
 * The translator's {@code ModuleLayer}, which replaces the JDK's in a translation with the JDK's
 * sources (see docs/reflection-and-plugins.md).
 *
 * <p>A translated program is closed: its one layer is the boot layer, whose modules are the
 * translation's named modules ({@link Module}), over the empty layer. A layer of more modules
 * would load code that is not in the translation, so defining one throws, saying so, and so does
 * {@link #configuration()}, the module graph that such a layer is resolved against (javac asks
 * for it only to load annotation processors from a module path).
 */
public final class ModuleLayer {
    private static final ModuleLayer EMPTY_LAYER = new ModuleLayer(List.of(), Set.of());
    private static ModuleLayer boot;

    private final List<ModuleLayer> parents;
    private final Set<Module> modules;

    private ModuleLayer(List<ModuleLayer> parents, Set<Module> modules) {
        this.parents = parents;
        this.modules = modules;
    }

    private static UnsupportedOperationException notInTranslation(String what) {
        return new UnsupportedOperationException(what + ": code that is not in the translation cannot run");
    }

    /** Java's: what defines a layer, which the translation does not make. */
    public static final class Controller {
        private Controller() {
        }

        public ModuleLayer layer() {
            throw notInTranslation("a module layer's controller");
        }

        public Controller addReads(Module source, Module target) {
            throw notInTranslation("a module layer's controller");
        }

        public Controller enableNativeAccess(Module target) {
            throw notInTranslation("a module layer's controller");
        }

        public Controller addExports(Module source, String pn, Module target) {
            throw notInTranslation("a module layer's controller");
        }

        public Controller addOpens(Module source, String pn, Module target) {
            throw notInTranslation("a module layer's controller");
        }
    }

    public ModuleLayer defineModulesWithOneLoader(Configuration cf, ClassLoader parentLoader) {
        throw notInTranslation("defining a module layer");
    }

    public ModuleLayer defineModulesWithManyLoaders(Configuration cf, ClassLoader parentLoader) {
        throw notInTranslation("defining a module layer");
    }

    public ModuleLayer defineModules(Configuration cf, Function<String, ClassLoader> clf) {
        throw notInTranslation("defining a module layer");
    }

    public static Controller defineModulesWithOneLoader(Configuration cf, List<ModuleLayer> parentLayers,
            ClassLoader parentLoader) {
        throw notInTranslation("defining a module layer");
    }

    public static Controller defineModulesWithManyLoaders(Configuration cf, List<ModuleLayer> parentLayers,
            ClassLoader parentLoader) {
        throw notInTranslation("defining a module layer");
    }

    public static Controller defineModules(Configuration cf, List<ModuleLayer> parentLayers,
            Function<String, ClassLoader> clf) {
        throw notInTranslation("defining a module layer");
    }

    public Configuration configuration() {
        throw notInTranslation("the configuration of a module layer, which resolves modules,");
    }

    public List<ModuleLayer> parents() {
        return parents;
    }

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

    public Set<Module> modules() {
        return modules;
    }

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

    boolean addEnableNativeAccess(String name) {
        throw notInTranslation("native access");
    }

    public ClassLoader findLoader(String name) {
        Optional<Module> module = findModule(name);
        if (module.isEmpty()) {
            throw new IllegalArgumentException("Module " + name + " not known to this layer");
        }
        return module.get().getClassLoader();
    }

    public String toString() {
        return modules.stream().map(Module::getName).collect(Collectors.joining(", "));
    }

    public static ModuleLayer empty() {
        return EMPTY_LAYER;
    }

    public static synchronized ModuleLayer boot() {
        if (boot == null) {
            boot = new ModuleLayer(List.of(EMPTY_LAYER),
                    Collections.unmodifiableSet(new LinkedHashSet<>(Module.named().values())));
        }
        return boot;
    }

    ServicesCatalog getServicesCatalog() {
        throw notInTranslation("the services catalog of a module layer");
    }

    void bindToLoader(ClassLoader loader) {
        throw notInTranslation("binding a module layer to a class loader");
    }

    static Stream<ModuleLayer> layers(ClassLoader loader) {
        throw notInTranslation("the module layers of a class loader");
    }
}
