package java.lang;

import java.lang.annotation.Annotation;
import java.net.URL;

/**
 * The translator's patch of the JDK's {@code Package} (see docs/jdk-overlays.md and
 * docs/reflection-and-plugins.md).
 *
 * <p>{@code Class.getPackage()} is {@code TranslatedModules.packageOf}, which makes one package
 * per name with {@link #ofTranslation}. A package of a named module has no versioning
 * information, as the JVM's class loaders define it. One of the unnamed module has the attributes
 * of the manifest of its root of the class path ({@code META-INF/MANIFEST.MF}, that of the jar
 * its classes come from), which the translator reads ({@code Module.manifest0}), or none when the
 * root has no manifest, as a directory of the class path has none. A sealed package is not
 * sealed: the translation has no jar's URL to seal it to. A package's annotations, which are
 * those of its {@code package-info} class, throw {@code UnsupportedOperationException}, saying
 * so; the lowering rejects them where it knows the package ({@code lower/packages.rs}), and
 * answers {@link #getPackage(String)} where it is called, if the answer is certain.
 */
// Not an `AnnotatedElement`: a package's annotations are not in the translation.
@Edit(find = "implements java.lang.reflect.AnnotatedElement", replace = "")
class Package {
    /** The package {@code name} of {@code module}, as {@code Class.getPackage()} gives it. */
    @Add
    static Package ofTranslation(String name, Module module) {
        String manifest = module.isNamed() ? null : Module.manifest0(name);
        if (manifest == null) {
            return new Package(name, module);
        }
        String[] values = new String[6];
        String[] names = {
            "Specification-Title", "Specification-Version", "Specification-Vendor",
            "Implementation-Title", "Implementation-Version", "Implementation-Vendor",
        };
        for (String line : manifest.split("\n")) {
            int colon = line.indexOf(": ");
            for (int i = 0; i < names.length; i++) {
                if (colon > 0 && names[i].equals(line.substring(0, colon))) {
                    values[i] = line.substring(colon + 2);
                }
            }
        }
        return new Package(name, values[0], values[1], values[2], values[3], values[4], values[5], null,
                module.getClassLoader());
    }

    // A package by name, and all of them, are those of the classes loaded so far: a call is
    // answered where it is made, if the answer is certain, or rejected (`lower/packages.rs`), so
    // these run only through a method reference. The JDK's ask their caller's class loader.
    @Replace
    @Deprecated(since = "9")
    public static Package getPackage(String name) {
        throw new UnsupportedOperationException("the package \"" + name
                + "\" by name, which depends on the classes loaded so far: code that is not in the translation cannot run");
    }

    @Replace
    public static Package[] getPackages() {
        throw new UnsupportedOperationException(
                "the packages loaded so far: code that is not in the translation cannot run");
    }

    @Add
    private UnsupportedOperationException annotationsNotInTranslation() {
        return new UnsupportedOperationException("the annotations of package \"" + packageName()
                + "\": code that is not in the translation cannot run");
    }

    // The JDK's finds the package's `package-info` class through its module's class loader.
    @Remove
    private Class<?> getPackageInfo();

    @Replace
    public <A extends Annotation> A getAnnotation(Class<A> annotationClass) {
        throw annotationsNotInTranslation();
    }

    @Replace
    public boolean isAnnotationPresent(Class<? extends Annotation> annotationClass) {
        throw annotationsNotInTranslation();
    }

    @Replace
    public <A extends Annotation> A[] getAnnotationsByType(Class<A> annotationClass) {
        throw annotationsNotInTranslation();
    }

    @Replace
    public Annotation[] getAnnotations() {
        throw annotationsNotInTranslation();
    }

    @Replace
    public <A extends Annotation> A getDeclaredAnnotation(Class<A> annotationClass) {
        throw annotationsNotInTranslation();
    }

    @Replace
    public <A extends Annotation> A[] getDeclaredAnnotationsByType(Class<A> annotationClass) {
        throw annotationsNotInTranslation();
    }

    @Replace
    public Annotation[] getDeclaredAnnotations() {
        throw annotationsNotInTranslation();
    }
}
