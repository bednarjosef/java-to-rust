public class SwitchFallThrough {
    static void run(int n) {
        switch (n) {
            case 1:
                System.out.println("one");
            case 2:
                System.out.println("two or fell from one");
                break;
            case 3:
                System.out.println("three");
            default:
                System.out.println("default, fell from three");
            case 4:
                System.out.println("four or fell from default");
        }
    }

    static int steps(String s) {
        int acc = 0;
        switch (s) {
            case "start":
                acc += 1;
            case "middle":
                acc += 10;
                if (acc > 100) {
                    break;
                }
            case "end":
                acc += 100;
                break;
            case "skip":
            default:
                acc = -1;
        }
        return acc;
    }

    static int countDown(int n) {
        int seen = 0;
        for (int i = n; i >= 0; i--) {
            switch (i % 4) {
                case 0:
                    seen += 1;
                case 1:
                    seen += 2;
                    continue;
                case 2:
                    if (i == 6) {
                        break;
                    }
                    seen += 4;
                default:
                    seen += 8;
            }
            seen += 100;
        }
        return seen;
    }

    public static void main(String[] args) {
        for (int n = 0; n <= 5; n++) {
            run(n);
        }
        System.out.println(steps("start"));
        System.out.println(steps("middle"));
        System.out.println(steps("end"));
        System.out.println(steps("skip"));
        System.out.println(steps("nothing"));
        System.out.println(countDown(9));
        int x = 0;
        switch (x) {
            case 0:
                x += 5;
            case 1:
                x += 7;
                break;
            case 2:
                x = 99;
        }
        System.out.println("x = " + x);
    }
}
