// A cast that cannot succeed, of a Named that is always a Circle to Square, throws; the local
// it initializes is still typed, as code after it in the never-taken branch uses it.
public class ImpossibleCastValue {
    interface Named {
        String name();
    }
    static class Square implements Named {
        int side = 2;
        public String name() {
            return "square";
        }
    }
    static class Circle implements Named {
        int radius = 3;
        public String name() {
            return "circle";
        }
    }
    static boolean never = false;

    public static void main(String[] args) {
        Named n = new Circle();
        Square t = new Square();
        if (never) {
            Square q = (Square) n;
            boolean same = q.equals(q);
            System.out.println(same + " " + q.side + " " + q.name());
        }
        System.out.println(n.name() + " " + t.side + " " + t.name());
    }
}
