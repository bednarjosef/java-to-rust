class Helper {
    static int made;

    static {
        System.out.println("Helper starts");
        made = 10;
    }

    static int make(int base) {
        System.out.println("make " + base);
        return base + made;
    }

    static int count() {
        made++;
        return made;
    }
}

public class StaticInit {
    static int seed;

    static {
        System.out.println("StaticInit starts");
        seed = 41;
    }

    static int next = Helper.make(seed);

    public static void main(String[] args) {
        System.out.println("main starts");
        System.out.println(next + " " + Helper.count());
        System.out.println(Helper.count());
    }
}
