public class Switch {
    static String day(int d) {
        switch (d) {
            case 1:
                return "Mon";
            case 2:
                return "Tue";
            case 6:
            case 7:
                return "weekend";
            default:
                return "midweek";
        }
    }

    static int weight(char c) {
        switch (c) {
            case 'a', 'e', 'i', 'o', 'u' -> {
                return 1;
            }
            case 'y' -> {
                return 2;
            }
            default -> {
                return 0;
            }
        }
    }

    static String size(byte b) {
        return switch (b) {
            case 0 -> "zero";
            case 1, 2, 3 -> "small";
            case -1 -> "minus one";
            default -> {
                if (b > 0) {
                    yield "big";
                }
                yield "negative";
            }
        };
    }

    static int code(short s) {
        int r = switch (s) {
            case 100 -> 1;
            case 200 -> 2;
            case -4 -> {
                int t = s * 3;
                yield t;
            }
            default -> 0;
        };
        return r;
    }

    static String color(String name) {
        switch (name) {
            case "red" -> {
                return "#f00";
            }
            case "green", "lime" -> {
                return "#0f0";
            }
            default -> {
                return "unknown";
            }
        }
    }

    static void report(String kind) {
        switch (kind) {
            case "log" -> System.out.println("writing a log");
            case "warn" -> System.out.println("writing a warning");
            default -> System.out.println("nothing to write");
        }
    }

    public static void main(String[] args) {
        for (int d = 0; d <= 7; d++) {
            System.out.println(d + " " + day(d));
        }
        String word = "queueing";
        int total = 0;
        for (int i = 0; i < word.length(); i++) {
            total += weight(word.charAt(i));
        }
        System.out.println("vowel weight " + total);
        System.out.println(size((byte) 0) + " " + size((byte) 2) + " " + size((byte) -1) + " " + size((byte) 9) + " " + size((byte) -9));
        System.out.println(code((short) 100) + " " + code((short) 200) + " " + code((short) -4) + " " + code((short) 7));
        System.out.println(color("red") + " " + color("lime") + " " + color("blue"));
        report("log");
        report("warn");
        report("other");
        int hits = 0;
        for (String s : new String[] {"a", "bb", "a"}) {
            switch (s) {
                case "a" -> hits += 10;
                default -> hits += 1;
            }
        }
        System.out.println("hits " + hits);
        String missing = null;
        System.out.println("before the null switch");
        switch (missing) {
            case "x" -> System.out.println("x");
            default -> System.out.println("default");
        }
        System.out.println("not reached");
    }
}
