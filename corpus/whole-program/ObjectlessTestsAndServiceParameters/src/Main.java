import java.util.ArrayList;
import java.util.List;
import java.util.ServiceLoader;
import plugins.Color;
import plugins.Shape;
import plugins.Unloaded;

// `instanceof` tests of classes that no code creates objects of, whose branches never run, next
// to tests of a class whose objects a subclass's constructor creates, of a record, of an
// exception and of a string. Then services loaded through a `Class` parameter that two static
// methods pass on, while the class literal of a service that is never loaded is used as a value.
public class Main {
    static class Never {
        String hello() {
            return "never";
        }
    }

    static class Later {
        String hello() {
            return "later";
        }
    }

    static class LaterSub extends Later {
        @Override
        String hello() {
            return "sub";
        }
    }

    record Point(int x, int y) {}

    static String describe(Object o) {
        if (o instanceof Never n) {
            return n.hello() + Tags.of(n);
        }
        if (o instanceof Later l && l.hello().length() > 1) {
            return "later " + l.hello();
        }
        if ((o instanceof Never)) {
            return "never again";
        } else if (o instanceof Point p) {
            return "point " + p.x() + " " + p.y();
        }
        if (o instanceof RuntimeException e) {
            return "exception " + e.getMessage();
        }
        return "other " + o;
    }

    static <S> List<String> names(Class<S> service) {
        return Loading.names(service);
    }

    public static void main(String[] args) {
        System.out.println(describe("text"));
        System.out.println(describe(new Point(1, 2)));
        System.out.println(describe(new IllegalStateException("boom")));
        System.out.println(describe(new LaterSub()));
        System.out.println(names(Shape.class));
        System.out.println(names(Color.class));
        System.out.println(Unloaded.class.getSimpleName());
    }
}

class Tags {
    static String of(Object o) {
        return " " + o.getClass().getName();
    }
}

class Loading {
    static <S> List<String> names(Class<S> service) {
        List<String> out = new ArrayList<>();
        for (S provider : ServiceLoader.load(service)) {
            out.add(provider.toString());
        }
        return out;
    }
}
