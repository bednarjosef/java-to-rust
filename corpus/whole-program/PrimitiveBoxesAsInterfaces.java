import java.io.Serializable;
import java.lang.constant.Constable;

// A primitive boxed straight into an interface that its box implements (`Serializable`,
// `Comparable<Double>`, `Constable`) is that box, and a cast of the interface back to the box
// gives the primitive again.
public class PrimitiveBoxesAsInterfaces {
    public static void main(String[] args) {
        Serializable s = 2.5;
        System.out.println(s);
        double d = (Double) s;
        System.out.println(d);
        Comparable<Double> c = 1.5;
        System.out.println(c.compareTo(2.0));
        System.out.println((Double) c);
        Constable k = 7L;
        System.out.println(k);
        long m = (Long) k;
        System.out.println(m);
        Serializable t = 'q';
        System.out.println(t);
        char ch = (Character) t;
        System.out.println(ch);
        Serializable b = true;
        boolean flag = (Boolean) b;
        System.out.println(flag + " " + b.equals(true));
    }
}
