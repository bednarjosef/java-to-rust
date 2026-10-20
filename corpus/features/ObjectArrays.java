class P {
    int x;
    P(int x) { this.x = x; }
    public String toString() { return "P" + x; }
}
public class ObjectArrays {
    public static void main(String[] args) {
        P[] ps = new P[3];
        ps[0] = new P(1);
        ps[2] = new P(3);
        int s = 0;
        for (P p : ps) if (p != null) s += p.x;
        System.out.println(s);
        System.out.println(ps[1] == null);
        System.out.println(ps[0] + " " + ps[1]);
        String[] names = new String[2];
        names[0] = "a";
        System.out.println(names[0] + names[1]);
        P[] lit = { new P(5), new P(6) };
        lit[0].x += 10;
        System.out.println(lit[0].x + lit[1].x);
        System.out.println(ps[1].x);
    }
}
