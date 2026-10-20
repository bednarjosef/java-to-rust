public class TailReturns {
    enum Color { RED, GREEN, BLUE }

    static String sign(int n) {
        if (n > 0) {
            return "pos";
        } else if (n < 0) {
            return "neg";
        } else {
            return "zero";
        }
    }

    static int nested(int n) {
        if (n > 0) {
            int doubled = n * 2;
            return doubled;
        } else {
            if (n == 0) {
                return 7;
            } else {
                return -1;
            }
        }
    }

    static int partial(int n) {
        if (n > 0) {
            return 1;
        }
        return 2;
    }

    static String day(int d) {
        switch (d) {
            case 1:
                return "Mon";
            case 2:
                return "Tue";
            default:
                return "midweek";
        }
    }

    static String open(int d) {
        switch (d) {
            case 1:
                return "one";
            case 2:
                break;
            default:
                return "many";
        }
        return "two";
    }

    static String fallThrough(int d) {
        switch (d) {
            case 1:
            case 2:
                return "low";
            default:
                return "high";
        }
    }

    static String color(Color c) {
        switch (c) {
            case RED:
                return "red";
            case GREEN:
                return "green";
            default:
                return "other";
        }
    }

    static String withLoop(int n) {
        if (n > 0) {
            for (int i = 0; i < n; i++) {
                if (i == 2) {
                    return "found " + i;
                }
            }
            return "short";
        } else {
            return "none";
        }
    }

    public static void main(String[] args) {
        System.out.println(sign(3) + " " + sign(-3) + " " + sign(0));
        System.out.println(nested(4) + " " + nested(0) + " " + nested(-4));
        System.out.println(partial(5) + " " + partial(-5));
        System.out.println(day(1) + " " + day(2) + " " + day(9));
        System.out.println(open(1) + " " + open(2) + " " + open(3));
        System.out.println(fallThrough(1) + " " + fallThrough(2) + " " + fallThrough(3));
        System.out.println(color(Color.RED) + " " + color(Color.GREEN) + " " + color(Color.BLUE));
        System.out.println(withLoop(5) + " " + withLoop(2) + " " + withLoop(0));
    }
}
