// Locals that javac's code sets in a loop followed by a switch that does not set them
// (`modifiersOpt`'s lastPos), in a `try` whose value is read after it (`Main`'s allArgs), and
// read through a field's array whose index calls a method.
public class LocalsSetInLoopsAndTries {
    static int pos = 0;
    int[] starts = {10, 20, 30};
    int line = 0;

    static int next() {
        return pos++;
    }

    int nextLine() {
        return line++;
    }

    int start() {
        return starts[nextLine()];
    }

    static int modifiers() {
        int flags = 0;
        int lastPos;
        loop:
        while (true) {
            int flag;
            switch (pos) {
                case 0: flag = 1; break;
                case 1: flag = 2; break;
                default: break loop;
            }
            lastPos = pos;
            next();
            if (flag == 2) {
                System.out.println("at " + lastPos);
            }
            flags |= flag;
        }
        switch (pos) {
            case 2: flags |= 8; break;
            default: break;
        }
        return flags;
    }

    static class Args {
        final String[] items;

        Args(String[] items) {
            this.items = items;
        }

        public String toString() {
            return String.join(" ", items);
        }
    }

    static Args parse(String[] argv) {
        if (argv.length > 2) {
            throw new IllegalArgumentException("too many");
        }
        return new Args(argv);
    }

    static int run(String[] argv) {
        Args allArgs;
        try {
            allArgs = parse(argv);
        } catch (IllegalArgumentException e) {
            System.out.println("bad: " + e.getMessage());
            return 2;
        }
        System.out.println(allArgs);
        return 0;
    }

    public static void main(String[] args) {
        System.out.println(modifiers());
        System.out.println(run(new String[] {"-g", "A.java"}) + " " + run(new String[] {"a", "b", "c"}));
        LocalsSetInLoopsAndTries t = new LocalsSetInLoopsAndTries();
        System.out.println(t.start() + " " + t.start());
    }
}
