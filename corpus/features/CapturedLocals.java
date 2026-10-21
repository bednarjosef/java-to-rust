public class CapturedLocals {
    interface Step {
        int next(int x);
    }

    static class Log {
        StringBuilder text = new StringBuilder();
        void add(String s) { text.append(s).append(';'); }
    }

    // A capture of a capture: an inner local class reads a local of the outer method through
    // the local class around it.
    static void nested(int offset) {
        String name = "outer";
        class Outer {
            int base = 10;
            String run(int k) {
                class Inner {
                    String show() { return name + "/" + (base + offset + k); }
                }
                return new Inner().show();
            }
        }
        Outer o = new Outer();
        System.out.println(o.run(1));
        o.base = 20;
        System.out.println(o.run(2));
    }

    // A local class whose method creates another local class of the same method, which
    // captures a local that the first never names.
    static void siblings(int limit) {
        String sep = ", ";
        class Range {
            String render(int from) {
                StringBuilder sb = new StringBuilder();
                for (int i = from; i < limit; i++) {
                    if (i > from) sb.append(sep);
                    sb.append(i);
                }
                return sb.toString();
            }
        }
        class Report {
            String make(int from) { return "[" + new Range().render(from) + "]"; }
        }
        System.out.println(new Report().make(2));
        System.out.println(new Report().make(limit - 1));
    }

    // A capture used after the object is created, and a captured local that is dead after the
    // capture.
    static void lifetimes() {
        String greeting = "hi";
        Log log = new Log();
        class Speaker implements Step {
            public int next(int x) {
                log.add(greeting + x);
                return x + greeting.length();
            }
        }
        Step s = new Speaker();
        int v = 0;
        for (int i = 0; i < 3; i++) {
            v = s.next(v);
        }
        log.add("done " + greeting);
        System.out.println(v + " " + log.text);
        String once = "only once";
        class Holder {
            String get() { return once.toUpperCase(); }
        }
        Holder h = new Holder();
        System.out.println(h.get());
    }

    // A local class whose objects are kept in an array and called later.
    static void kept() {
        int[] totals = new int[2];
        class Adder implements Step {
            final int by;
            Adder(int by) { this.by = by; }
            public int next(int x) {
                totals[by % 2] += by;
                return x + by;
            }
        }
        Step[] steps = { new Adder(1), new Adder(2), new Adder(5) };
        int x = 0;
        for (Step st : steps) {
            x = st.next(x);
        }
        System.out.println(x + " " + totals[0] + " " + totals[1]);
    }

    public static void main(String[] args) {
        nested(5);
        siblings(6);
        lifetimes();
        kept();
    }
}
