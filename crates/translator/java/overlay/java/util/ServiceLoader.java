package java.util;

import java.util.function.Supplier;
import java.util.stream.Stream;

/**
 * The translator's {@code ServiceLoader}, which replaces the JDK's in a translation with the
 * JDK's sources (see docs/reflection-and-plugins.md).
 *
 * <p>A translated program is closed: the providers of a service are those that its sources and
 * libraries declare, with {@code provides S with P;} in a {@code module-info.java} or in a file
 * {@code META-INF/services/S} of a root, in that order, which the translator reads when it
 * translates (the natives {@code providers0} and {@code create0}). A provider is made as Java
 * makes it, by its constructor without parameters, or a module's by its public static
 * {@code provider()} method if it has one, when the loader's iterator first reaches it; the
 * iterators keep the providers they made, which {@link #findFirst()} gives too, and
 * {@link #reload()} forgets them, while a {@link Provider}'s {@code get()} makes a new one. A class
 * loader that would load code from outside the translation, such as a {@code URLClassLoader}
 * over a directory with providers of the service, fails loudly when it is first iterated
 * ({@code check}); one over nothing finds no more than the translation has. A module layer is not in the translation, so
 * {@link #load(ModuleLayer, Class)} throws.
 */
public final class ServiceLoader<S> implements Iterable<S> {
    /** A provider that a loader found, which makes its object when asked. */
    public static interface Provider<S> extends Supplier<S> {
        Class<? extends S> type();

        @Override
        S get();
    }

    private final Class<S> service;
    private final ClassLoader loader;
    private final List<Provider<S>> providers = new ArrayList<>();
    /** The providers that the iterators made so far, in order. */
    private final List<S> made = new ArrayList<>();
    /** Whether the loader was found to load no code from outside the translation ({@code check}). */
    private boolean checked;

    private ServiceLoader(Class<S> service, ClassLoader loader) {
        this.service = Objects.requireNonNull(service);
        this.loader = loader;
        reload();
    }

    public static <S> ServiceLoader<S> load(Class<S> service, ClassLoader loader) {
        return new ServiceLoader<>(service, loader);
    }

    // `ResourceBundle`'s, for a named module's bundles: the translation's providers too.
    static <S> ServiceLoader<S> load(Class<S> service, ClassLoader loader, Module callerModule) {
        return new ServiceLoader<>(service, loader);
    }

    public static <S> ServiceLoader<S> load(Class<S> service) {
        return new ServiceLoader<>(service, null);
    }

    public static <S> ServiceLoader<S> loadInstalled(Class<S> service) {
        return new ServiceLoader<>(service, null);
    }

    public static <S> ServiceLoader<S> load(ModuleLayer layer, Class<S> service) {
        throw new UnsupportedOperationException("ServiceLoader.load(ModuleLayer, " + service.getName()
                + "): code that is not in the translation cannot run, and a module layer loads such code");
    }

    public Iterator<S> iterator() {
        return new Iterator<S>() {
            private int next;

            public boolean hasNext() {
                check();
                return next < providers.size();
            }

            public S next() {
                check();
                if (next >= providers.size()) {
                    throw new NoSuchElementException();
                }
                while (made.size() <= next) {
                    made.add(providers.get(made.size()).get());
                }
                S provider = made.get(next);
                next++;
                return provider;
            }
        };
    }

    public Stream<Provider<S>> stream() {
        check();
        return new ArrayList<>(providers).stream();
    }

    // The iterators' first provider, as Java finds it.
    public Optional<S> findFirst() {
        Iterator<S> iterator = iterator();
        return iterator.hasNext() ? Optional.of(iterator.next()) : Optional.empty();
    }

    public void reload() {
        providers.clear();
        made.clear();
        checked = false;
        Class<?>[] types = providers0(service.getName());
        for (int i = 0; i < types.length; i++) {
            providers.add(new Found<>(service, types[i]));
        }
    }

    /**
     * Fails when the loader would find providers of the service outside the translation: a
     * {@code URLClassLoader} over a directory that declares some.
     */
    private void check() {
        if (checked) {
            return;
        }
        if (loader instanceof java.net.URLClassLoader) {
            String where = ((java.net.URLClassLoader) loader).outside("META-INF/services/" + service.getName());
            if (where != null) {
                throw new ServiceConfigurationError(service.getName() + ": the providers that " + where
                        + " declares are code that is not in the translation, which cannot run");
            }
        }
        checked = true;
    }

    public String toString() {
        return "java.util.ServiceLoader[" + service.getName() + "]";
    }

    /** A provider of the translation. */
    private static final class Found<S> implements Provider<S> {
        private final Class<S> service;
        private final Class<?> type;

        Found(Class<S> service, Class<?> type) {
            this.service = service;
            this.type = type;
        }

        @SuppressWarnings("unchecked")
        public Class<? extends S> type() {
            return (Class<? extends S>) type;
        }

        public S get() {
            return service.cast(create0(type));
        }
    }

    /** The classes of the providers of the service with this binary name, in the loader's order. */
    private static native Class<?>[] providers0(String service);

    /** An object of the provider class {@code type}, made by its {@code provider()} or constructor. */
    private static native Object create0(Class<?> type);
}
