// Object values here are all objects of Made, so they are its handles; an Unused value that
// becomes an Object can only be null, as no Unused object is ever created.
public class UncreatedClassUpcast {
    interface Sized {
        int size();
        default int twice() {
            return size() * 2;
        }
    }
    static class Base implements Sized {
        int k = 1;
        public int size() {
            return k;
        }
        int grow() {
            return k + 1;
        }
    }
    static class Made extends Base {
    }
    static class Unused extends Base implements Sized {
        @Override
        public boolean equals(Object o) {
            if (this == o) {
                return true;
            }
            return o instanceof Unused u && u.k == k;
        }
        @Override
        public int hashCode() {
            return k;
        }
        int both() {
            return grow() + twice();
        }
    }
    static Unused never = null;

    public static void main(String[] args) {
        Base b = new Made();
        Sized s = b;
        Object o = b;
        System.out.println(b.grow() + " " + s.twice() + " " + o.equals(b));
        if (never != null) {
            System.out.println(never.both() + " " + never.equals(o));
        }
        Object maybe = never;
        System.out.println(maybe == null);
    }
}
