public class ArrayCopyTexts {
    static class Box {}

    // Copies one element and prints the ArrayStoreException's message, or "ok".
    static void copy(Object src, Object dest) {
        try {
            System.arraycopy(src, 0, dest, 0, 1);
            System.out.println("ok");
        } catch (ArrayStoreException e) {
            System.out.println(e.getMessage());
        }
    }

    // Copies two elements; an element that does not fit stops the copy after the ones before it.
    static void copyTwo(Object src, Object dest) {
        try {
            System.arraycopy(src, 0, dest, 0, 2);
            System.out.println("ok");
        } catch (ArrayStoreException e) {
            System.out.println(e.getMessage());
        }
    }

    // No catch: the exception is uncaught, and the program exits with code 1.
    static void plain(Object src, Object dest) {
        System.arraycopy(src, 0, dest, 0, 2);
        System.out.println("not reached");
    }

    public static void main(String[] args) {
        Object ints = new int[1];
        Object longs = new long[1];
        Object strings = new String[1];
        Object objects = new Object[1];
        Object text = "s";
        Object box = new Box();
        copy(ints, ints);
        copy(ints, strings);
        copy(ints, objects);
        copy(ints, longs);
        copy(longs, ints);
        copy(strings, ints);
        copy(objects, longs);
        copy(longs, objects);
        copy(longs, longs);
        copy(text, ints);
        copy(box, strings);
        copy(ints, text);
        copy(ints, box);
        copy(strings, objects);
        copy(objects, strings);
        Object[] refs = {"a", new Box()};
        String[] words = new String[2];
        copyTwo(refs, words);
        System.out.println("words " + words[0] + " " + words[1]);
        Object[] fine = {"b", null};
        copyTwo(fine, words);
        System.out.println("words " + words[0] + " " + words[1]);
        plain(refs, words);
    }
}
