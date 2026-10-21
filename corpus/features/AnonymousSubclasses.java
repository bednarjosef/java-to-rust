public class AnonymousSubclasses {
    static abstract class Report {
        final String title;
        int lines;

        Report(String title, int lines) {
            this.title = title;
            this.lines = lines;
        }

        abstract String line(int i);

        // The base calls the method that each anonymous subclass overrides.
        String render() {
            StringBuilder sb = new StringBuilder(title).append(':');
            for (int i = 0; i < lines; i++) sb.append(' ').append(line(i));
            return sb.toString();
        }
    }

    static class Counter {
        int count;
        int next() { return ++count; }
        String name() { return "counter"; }
    }

    public static void main(String[] args) {
        String sep = "-";
        // Constructor arguments passed on to the superclass, an instance initializer and a field.
        Report squares = new Report("squares", 3) {
            int calls;
            {
                lines++;
            }
            String line(int i) {
                calls++;
                return i * i + sep + calls;
            }
        };
        System.out.println(squares.render());
        System.out.println(squares.render());

        int start = 5;
        Report countdown = new Report("down", start) {
            String line(int i) { return String.valueOf(start - i); }
        };
        System.out.println(countdown.render());

        // An anonymous subclass of a concrete class, overriding one method and keeping another.
        Counter evens = new Counter() {
            int next() { count += 2; return count; }
            String name() { return "evens"; }
        };
        Counter plain = new Counter();
        Counter[] counters = { evens, plain };
        for (Counter c : counters) {
            c.next();
            System.out.println(c.name() + " " + c.next());
        }
        System.out.println(evens.getClass().getSimpleName().isEmpty() + " " + plain.getClass().getSimpleName());
    }
}
