public class LocalMemberClasses {
    interface Shared {
        int value();
    }

    static int run(int k) {
        class Counter {
            int count = k;

            record Step(int by) {}

            enum Mode { UP, DOWN }

            interface Rule {
                int apply(int n);
            }

            class Tally {
                int total() {
                    return count * 2 + k;
                }
            }

            static class Helper {
                static String name() {
                    return "helper";
                }
            }

            int next() {
                Step s = new Step(3);
                Rule r = n -> n + s.by();
                count = r.apply(count);
                return count;
            }
        }
        Counter c = new Counter();
        System.out.println(c.next() + " " + c.new Tally().total());
        System.out.println(Counter.Mode.UP + " " + Counter.Helper.name() + " " + new Counter.Step(5));

        Shared anon = new Shared() {
            class Inner {
                int twice() {
                    return k * 2;
                }
            }

            public int value() {
                return new Inner().twice();
            }
        };
        System.out.println(anon.value());
        return c.next();
    }

    public static void main(String[] args) {
        System.out.println(run(4));
    }
}
