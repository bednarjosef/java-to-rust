// An argument that assigns a field of `this` runs before the call borrows `self`, even for a
// method that takes `&self`, which two-phase borrows do not cover.
public class ArgumentsChangeThis {
    static long sLong = 2;
    static class Base {
        int f12 = 3;
        int ov(int a0) {
            return f12 + a0;
        }
    }
    static class Sub extends Base {
        String f17 = "s";
        int g2(int a0) {
            return a0 + ov((f12 <<= sLong));
        }
        int g3() {
            return ov(f12++) + ov(f12 += 2);
        }
        String g4() {
            return name(f17 += "!");
        }
        String name(String s) {
            return s + f17;
        }
    }

    public static void main(String[] args) {
        Sub s = new Sub();
        System.out.println(s.g2(1) + " " + s.f12);
        System.out.println(s.g3() + " " + s.f12);
        System.out.println(s.g4() + " " + s.f17);
    }
}
