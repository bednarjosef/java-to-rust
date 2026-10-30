package sun.security.jca;

import java.lang.reflect.InvocationTargetException;

/**
 * What the JDK's security providers find by name at run time, by reflection or in the JDK's
 * files, for a translated program, which has neither (see docs/jdk-overlays.md): the classes of
 * the services that a provider makes ({@code Provider.Service.newInstance}), the key interfaces
 * that it names, and the security properties.
 */
public final class TranslatedServices {
    private TranslatedServices() {}

    /** What {@link #construct} gives for a name of no class of the translation. */
    private static final Object NO_CLASS = new Object();

    /** What {@link #construct} gives for a class without the constructor asked for. */
    private static final Object NO_CONSTRUCTOR = new Object();

    /**
     * A new object of the class with the binary name {@code className}, which a provider
     * registers as the implementation of a service ({@code Provider.Service.newInstance}): made
     * with its public constructor without parameters when {@code parameterClass} is null, else
     * with the one whose only parameter is of that class, given {@code parameter}. The JDK finds
     * the class by name and its constructor by reflection; the translation is closed, so the
     * classes are those the providers register for the services that reachable code looks up,
     * and that are translated ({@code program/reach/services.rs}). As with reflection, a name of
     * no such class throws {@code ClassNotFoundException}, a class without that constructor
     * {@code NoSuchMethodException}, and an exception of the constructor is wrapped in an
     * {@code InvocationTargetException}.
     */
    public static Object newInstance(String className, Class<?> parameterClass, Object parameter)
            throws ClassNotFoundException, NoSuchMethodException, InvocationTargetException {
        String parameterName = parameterClass == null ? "" : parameterClass.getName();
        Object made;
        try {
            made = construct(className, parameterName, parameter, NO_CLASS, NO_CONSTRUCTOR);
        } catch (Throwable e) {
            throw new InvocationTargetException(e);
        }
        if (made == NO_CLASS) {
            throw new ClassNotFoundException(className + ", which is not in the translation");
        }
        if (made == NO_CONSTRUCTOR) {
            throw new NoSuchMethodException(className + ".<init>(" + parameterName + ")");
        }
        return made;
    }

    /**
     * The object of {@link #newInstance}, {@code noClass} or {@code noConstructor}: the translator
     * writes this method's calls as a {@code switch} over the class name with a case for each
     * class, and in it one over {@code parameterName} ({@code ""} for none) with a case for each
     * constructor ({@code lower/by_name.rs}).
     */
    private static Object construct(String className, String parameterName, Object parameter,
            Object noClass, Object noConstructor) {
        return noClass;
    }

    /**
     * The text of the JDK's security properties file, {@code conf/security/java.security}, which
     * {@code Security} loads first: the translator writes this method's calls as the text of the
     * file of the JDK it translates with ({@code lower/system.rs}), as a translated program has
     * no JDK to read it from at run time; null if that JDK has none.
     */
    public static String securityProperties() {
        return null;
    }
}
