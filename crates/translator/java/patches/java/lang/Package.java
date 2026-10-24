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
 * of the manifest of the jar its classes were loaded from, or none when they were loaded from a
 * directory: the translation does not know which, so asking for them (the titles, versions and
 * vendors, whether it is sealed, {@code isCompatibleWith} and {@code toString}, which shows the
 * specification's title and version) throws {@code UnsupportedOperationException}, saying so.
 * So do a package's annotations, which are those of its {@code package-info} class. The lowering
 * rejects both where it knows the package ({@code lower/packages.rs}), and answers
 * {@link #getPackage(String)} where it is called, if the answer is certain.
 */
// Not an `AnnotatedElement`: a package's annotations are not in the translation.
@Edit(find = "implements java.lang.reflect.AnnotatedElement", replace = "")
class Package {
    /** Whether the package's versioning information is a manifest's that is not in the translation. */
    @Add
    private boolean manifestUnknown;

    /** The package {@code name} of {@code module}, as {@code Class.getPackage()} gives it. */
    @Add
    static Package ofTranslation(String name, Module module) {
        Package p = new Package(name, module);
        p.manifestUnknown = !module.isNamed();
        return p;
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

    /** The package's versioning information, if the translation knows it. */
    @Add
    private VersionInfo versionInfo() {
        if (manifestUnknown) {
            throw new UnsupportedOperationException("the manifest attributes of package \"" + packageName()
                    + "\": code that is not in the translation cannot run");
        }
        return versionInfo;
    }

    @Edit(find = "versionInfo.specTitle", replace = "versionInfo().specTitle")
    public String getSpecificationTitle();

    @Edit(find = "versionInfo.specVersion", replace = "versionInfo().specVersion")
    public String getSpecificationVersion();

    @Edit(find = "versionInfo.specVendor", replace = "versionInfo().specVendor")
    public String getSpecificationVendor();

    @Edit(find = "versionInfo.implTitle", replace = "versionInfo().implTitle")
    public String getImplementationTitle();

    @Edit(find = "versionInfo.implVersion", replace = "versionInfo().implVersion")
    public String getImplementationVersion();

    @Edit(find = "versionInfo.implVendor", replace = "versionInfo().implVendor")
    public String getImplementationVendor();

    @Edit(find = "versionInfo.sealBase", replace = "versionInfo().sealBase")
    public boolean isSealed();

    @Edit(find = "if (versionInfo !=", replace = "if (versionInfo() !=")
    public boolean isSealed(URL url);

    @Edit(find = "if (versionInfo.specVersion == null", replace = "if (versionInfo().specVersion == null")
    public boolean isCompatibleWith(String desired);

    @Edit(find = "String spec = versionInfo.specTitle;", replace = "String spec = versionInfo().specTitle;")
    @Edit(find = "String ver =  versionInfo.specVersion;", replace = "String ver = versionInfo().specVersion;")
    public String toString();

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
