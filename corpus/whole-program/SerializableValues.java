import java.io.Serializable;
import java.util.HashMap;
import java.util.Map;

// Classes of the program that implement the JDK's marker interface `Serializable`, held as
// one, as java.time's TzdbZoneRulesProvider holds the ZoneRules that its Ser.read returns: a
// switch that yields them as `Serializable`, kept in a map of `Object`s and cast back, and
// `instanceof Serializable` of them and of an object that is not one.
public class SerializableValues {
    static final class Rules implements Serializable {
        final int offset;

        Rules(int offset) {
            this.offset = offset;
        }

        static Rules readExternal(int offset) {
            return new Rules(offset);
        }
    }

    static final class Transition implements Serializable {
        static Transition readExternal() {
            return new Transition();
        }
    }

    static final class Plain {}

    static Serializable read(byte type, int offset) {
        return switch (type) {
            case 1 -> Rules.readExternal(offset);
            case 2 -> Transition.readExternal();
            default -> throw new IllegalArgumentException("unknown " + type);
        };
    }

    static final Map<String, Object> rules = new HashMap<>();

    static Rules provide(String id) {
        Object obj = rules.get(id);
        if (obj instanceof byte[] bytes) {
            obj = read((byte) 1, bytes.length);
            rules.put(id, obj);
        }
        return (Rules) obj;
    }

    public static void main(String[] args) {
        rules.put("a", new byte[3]);
        System.out.println(provide("a").offset + " " + provide("a").offset);
        Object[] values = {read((byte) 2, 0), new Plain(), provide("a")};
        for (Object v : values) {
            System.out.println(v.getClass().getSimpleName() + " " + (v instanceof Serializable));
        }
    }
}
