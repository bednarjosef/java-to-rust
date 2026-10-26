// An annotation whose values are constants of other classes that no code reads, as log4j's
// `@Plugin(name = ConsoleAppender.PLUGIN_NAME, elementType = Appender.ELEMENT_TYPE)`, and a
// method that returns `"true".equalsIgnoreCase(s) || d && !"false".equalsIgnoreCase(s)` for a
// string that may be null, as log4j's `Booleans.parseBoolean`.
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.TYPE)
@interface Plugin {
    String name();
    String category();
    String elementType() default "";
    boolean printObject() default false;
}

public class PluginAnnotationConstants {
    interface Appender {
        String ELEMENT_TYPE = "appender";
    }

    static final class Core {
        static final String CATEGORY_NAME = "Core";
    }

    @Plugin(name = ConsoleAppender.PLUGIN_NAME, category = Core.CATEGORY_NAME,
            elementType = Appender.ELEMENT_TYPE, printObject = ConsoleAppender.PRINTS)
    static final class ConsoleAppender implements Appender {
        static final String PLUGIN_NAME = "Console";
        static final boolean PRINTS = true;
    }

    static boolean parseBoolean(String s, boolean defaultValue) {
        return "true".equalsIgnoreCase(s) || defaultValue && !"false".equalsIgnoreCase(s);
    }

    public static void main(String[] args) {
        Plugin p = ConsoleAppender.class.getAnnotation(Plugin.class);
        System.out.println(p.name() + " " + p.category() + " " + p.elementType() + " " + p.printObject());
        String none = args.length > 5 ? args[0] : null;
        System.out.println(parseBoolean("TRUE", false) + " " + parseBoolean(none, true) + " "
                + parseBoolean("false", true) + " " + parseBoolean(none, false));
    }
}
