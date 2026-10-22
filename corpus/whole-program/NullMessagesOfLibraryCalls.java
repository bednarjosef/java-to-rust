// The messages of the NullPointerExceptions of calls that the translation writes itself: toString
// of an Object or an interface, an enum's own methods, case in a null locale (the JDK's code
// throws one without a message), a String of null code points, EnumSet.of a null, and the JDK's
// call on a value of a type variable (`K extends Comparable`), named by the variable's bound.
import java.util.AbstractMap;
import java.util.EnumSet;
import java.util.Locale;
import java.util.Map;
import java.util.concurrent.TimeUnit;

public class NullMessagesOfLibraryCalls {
    enum Color { RED, GREEN }

    interface Shape {
        int sides();
    }

    static Object object(int i) {
        return i > 100 ? "x" : null;
    }

    static Shape shape(int i) {
        return i > 100 ? () -> 3 : null;
    }

    static Color color(int i) {
        return i > 100 ? Color.RED : null;
    }

    static Locale locale(int i) {
        return i > 100 ? Locale.US : null;
    }

    static int[] points(int i) {
        return i > 100 ? new int[] {65} : null;
    }

    static void run(int which) {
        try {
            switch (which) {
                case 0 -> System.out.println(object(which).toString());
                case 1 -> System.out.println(shape(which).toString());
                case 2 -> System.out.println(color(which).ordinal());
                case 3 -> System.out.println(color(which).name());
                case 4 -> System.out.println("Abc".toLowerCase(locale(which)));
                case 5 -> System.out.println("Abc".toUpperCase(locale(which)));
                case 6 -> System.out.println(new String(points(which), 0, 1));
                case 7 -> System.out.println(EnumSet.of(TimeUnit.SECONDS, (TimeUnit) null));
                case 8 -> System.out.println(color(which).compareTo(Color.RED));
                default -> System.out.println(Map.Entry.<String, Integer>comparingByKey()
                        .compare(new AbstractMap.SimpleEntry<>(null, 1), new AbstractMap.SimpleEntry<>("a", 2)));
            }
        } catch (NullPointerException e) {
            System.out.println(which + ": " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        for (int i = 0; i < 10; i++) {
            run(i);
        }
    }
}
