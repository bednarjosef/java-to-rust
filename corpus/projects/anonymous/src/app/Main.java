package app;

import extra.Audit;

/**
 * Anonymous and local classes are reached as classes are: an anonymous class is created where its
 * `new` runs, and only its methods that a call may run are translated. `describe` and `audit` are
 * never called, so the package `extra` that only they use is not translated.
 */
public class Main {
    interface Task {
        String run();

        String audit();
    }

    interface Shape {
        int area();

        default String describe() {
            return "shape " + area();
        }
    }

    static Task task(String name) {
        return new Task() {
            int runs = 0;

            public String run() {
                runs++;
                return name + " ran " + runs + " time" + (runs == 1 ? "" : "s");
            }

            public String audit() {
                return Audit.record(name);
            }

            public String toString() {
                return "task " + name;
            }
        };
    }

    /** Never called: the anonymous class inside is not created. */
    static Task never() {
        return new Task() {
            public String run() {
                return Audit.record("never");
            }

            public String audit() {
                return Audit.record("never");
            }
        };
    }

    public static void main(String[] args) {
        Task build = task("build");
        System.out.println(build.run());
        System.out.println(build.run());
        System.out.println(build);
        class Square implements Shape {
            final int side;

            Square(int side) {
                this.side = side;
            }

            public int area() {
                return side * side;
            }
        }
        Shape square = new Square(4);
        System.out.println(square.area());
        Shape unit = new Shape() {
            public int area() {
                return 1;
            }
        };
        System.out.println(unit.area() + square.area());
    }
}
