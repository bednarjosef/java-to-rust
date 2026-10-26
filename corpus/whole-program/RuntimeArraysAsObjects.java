// Arrays held as `Object`s and tested, cast and asked for their class: arrays of the runtime's
// classes (`String[]`, `Class[]`, boxes), of the program's classes, of primitives, and arrays of
// arrays, which are `Object[]`s too where an array of primitives is not. (A `new Object[] {..}`
// carries no class, and passes every test of an array class: see docs/object-model.md.)
public class RuntimeArraysAsObjects {
    static class Point {
        final int x;

        Point(int x) {
            this.x = x;
        }
    }

    static String describe(Object o) {
        StringBuilder out = new StringBuilder();
        out.append(o instanceof Object[]).append(' ');
        out.append(o instanceof String[]).append(' ');
        out.append(o instanceof Integer[]).append(' ');
        out.append(o instanceof Class<?>[]).append(' ');
        out.append(o instanceof Point[]).append(' ');
        out.append(o instanceof int[]).append(' ');
        out.append(o instanceof int[][]).append(' ');
        out.append(o instanceof Object[][]).append(' ');
        out.append(o instanceof Comparable<?>[]).append(' ');
        Class<?> c = o.getClass();
        out.append(c.isArray()).append(' ').append(c.getSimpleName());
        Class<?> component = c.getComponentType();
        out.append(' ').append(component == null ? "none" : component.getName());
        if (o instanceof Object[] objects) {
            out.append(" length ").append(objects.length);
            if (objects.length > 0) {
                Object first = objects[0];
                String shown = first instanceof Object[] || first instanceof int[] ? "an array"
                        : first instanceof Point p ? "point " + p.x
                        : String.valueOf(first);
                out.append(" first ").append(shown);
            }
        }
        return out.toString();
    }

    static String cast(Object o) {
        try {
            Object[] objects = (Object[]) o;
            return "Object[] of " + objects.length;
        } catch (ClassCastException e) {
            return "not an Object[]";
        }
    }

    public static void main(String[] args) {
        Object[] values = {
            new String[] {"a", "b"},
            new Class<?>[] {String.class},
            new Integer[] {1, 2, 3},
            new Long[] {4L},
            new Double[] {2.5},
            new Character[] {'c'},
            new Boolean[] {true},
            new Byte[] {(byte) 1},
            new Short[] {(short) 2},
            new Float[] {1.5f},
            new StringBuilder[] {new StringBuilder("sb")},
            new Point[] {new Point(1)},
            new int[] {1, 2},
            new long[] {3},
            new char[] {'x'},
            new boolean[] {false},
            new double[] {0.5},
            new int[][] {{1}, {2, 3}},
            new String[][] {{"x"}},
            new Object[][] {{"o"}},
            new Comparable<?>[] {"c", 1},
        };
        for (Object v : values) {
            System.out.println(describe(v));
            System.out.println(cast(v));
        }
        Object strings = new String[] {"p", "q"};
        String[] back = (String[]) strings;
        System.out.println(back[1] + " " + ((Object[]) strings).length);
        Object rows = new int[][] {{7, 8}};
        int[] row = ((int[][]) rows)[0];
        System.out.println(row[1] + " " + (((Object[]) rows)[0] instanceof int[]));
        try {
            Integer[] wrong = (Integer[]) strings;
            System.out.println("cast " + wrong.length);
        } catch (ClassCastException e) {
            System.out.println("ClassCastException");
        }
    }
}
