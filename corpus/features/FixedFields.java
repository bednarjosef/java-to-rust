// Fields that only constructors write, read in methods while the object's other fields change:
// final ones, inherited ones, one written by a field initializer, and one a constructor reads
// through a method before it writes it (and sees null).
public class FixedFields {
    static class Base {
        final String name;
        final StringBuilder log = new StringBuilder();
        String early;

        Base(String name) {
            early = describe();
            this.name = name;
        }

        String describe() {
            return "base " + name;
        }
    }

    static class Counter extends Base {
        private final int step;
        private Counter next;
        private int count;

        Counter(String name, int step, Counter next) {
            super(name);
            this.step = step;
            this.next = next;
        }

        void tick() {
            count += step;
            log.append(name).append("=").append(count).append(' ');
            if (next != null) {
                next.tick();
            }
        }

        int total() {
            return count + (next == null ? 0 : next.total());
        }
    }

    public static void main(String[] args) {
        Counter c = new Counter("a", 2, new Counter("b", 3, null));
        for (int i = 0; i < 3; i++) {
            c.tick();
        }
        System.out.println(c.log + " " + c.next.log);
        System.out.println(c.total() + " " + c.early + " " + c.next.name.length());
    }
}
