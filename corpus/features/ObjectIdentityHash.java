// System.identityHashCode of an Object or interface value hashes the object its variant holds:
// the same object gives the same hash, null gives 0, and a plain `new Object()` hashes as hashCode does.
public class ObjectIdentityHash {
    interface Named {
        String name();
    }
    static class Point implements Named {
        int x;
        Point(int x) {
            this.x = x;
        }
        public String name() {
            return "p" + x;
        }
        @Override
        public int hashCode() {
            return x;
        }
    }
    static class Line implements Named {
        public String name() {
            return "line";
        }
    }

    public static void main(String[] args) {
        Object text = "text";
        Object box = 42;
        Object point = new Point(3);
        Object same = point;
        Named named = new Line();
        Named other = new Point(4);
        int h = System.identityHashCode(point);
        System.out.println((h == System.identityHashCode(same)) + " " + (h != 0));
        System.out.println(System.identityHashCode(text) == System.identityHashCode(text));
        System.out.println(System.identityHashCode(box) == System.identityHashCode(box));
        System.out.println((System.identityHashCode(named) != 0) + " " + (System.identityHashCode(other) == other.hashCode()));
        Object none = null;
        Named nobody = null;
        System.out.println(System.identityHashCode(none) + " " + System.identityHashCode(nobody));
        System.out.println(((Named) point).name() + " " + named.name());
        Object plain = new Object();
        Object alias = plain;
        Object another = new Object();
        int p = System.identityHashCode(plain);
        System.out.println((p == System.identityHashCode(alias)) + " " + (p == plain.hashCode()));
        System.out.println(System.identityHashCode(another) == another.hashCode());
    }
}
