// `System.arraycopy` between arrays of classes other than `String` and `Object`. A copy into an
// array whose element class holds every element of the source's is plain; another copy checks
// each element as it is stored, so a null always passes, and the first element of another class
// throws `ArrayStoreException` after the elements before it are stored.
public class ArrayCopyClasses {
    interface Named {
        String name();
    }

    static class Box implements Named {
        final int id;

        Box(int id) {
            this.id = id;
        }

        public String name() {
            return "box" + id;
        }

        public String toString() {
            return name();
        }
    }

    static class Base {
        public String toString() {
            return "Base";
        }
    }

    static class Sub extends Base {
        public String toString() {
            return "Sub";
        }
    }

    static class Plain {
        public String toString() {
            return "Plain";
        }
    }

    // One helper per element class: an array passed as an `Object[]` would be one everywhere.
    static String showObjects(Object[] xs) {
        StringBuilder sb = new StringBuilder("[");
        for (int i = 0; i < xs.length; i++) {
            sb.append(i == 0 ? "" : ", ").append(xs[i]);
        }
        return sb.append("]").toString();
    }

    static String showBoxes(Box[] xs) {
        StringBuilder sb = new StringBuilder("[");
        for (int i = 0; i < xs.length; i++) {
            sb.append(i == 0 ? "" : ", ").append(xs[i]);
        }
        return sb.append("]").toString();
    }

    static String showBases(Base[] xs) {
        StringBuilder sb = new StringBuilder("[");
        for (int i = 0; i < xs.length; i++) {
            sb.append(i == 0 ? "" : ", ").append(xs[i]);
        }
        return sb.append("]").toString();
    }

    static String showSubs(Sub[] xs) {
        StringBuilder sb = new StringBuilder("[");
        for (int i = 0; i < xs.length; i++) {
            sb.append(i == 0 ? "" : ", ").append(xs[i]);
        }
        return sb.append("]").toString();
    }

    static String showNamed(Named[] xs) {
        StringBuilder sb = new StringBuilder("[");
        for (int i = 0; i < xs.length; i++) {
            sb.append(i == 0 ? "" : ", ").append(xs[i]);
        }
        return sb.append("]").toString();
    }

    static String showPlains(Plain[] xs) {
        StringBuilder sb = new StringBuilder("[");
        for (int i = 0; i < xs.length; i++) {
            sb.append(i == 0 ? "" : ", ").append(xs[i]);
        }
        return sb.append("]").toString();
    }

    public static void main(String[] args) {
        Box[] boxes = {new Box(1), new Box(2)};
        Object[] objects = new Object[3];
        System.arraycopy(boxes, 0, objects, 1, 2);
        System.out.println(showObjects(objects));

        Object[] all = {new Box(3), null, new Box(4)};
        Box[] back = new Box[3];
        System.arraycopy(all, 0, back, 0, 3);
        System.out.println(showBoxes(back));

        Object[] mixed = {new Box(5), new Box(6), "text", new Box(7)};
        Box[] partial = new Box[4];
        try {
            System.arraycopy(mixed, 0, partial, 0, 4);
        } catch (ArrayStoreException e) {
            System.out.println(e.getMessage());
        }
        System.out.println(showBoxes(partial));

        Sub[] subs = {new Sub(), new Sub()};
        Base[] bases = new Base[3];
        System.arraycopy(subs, 0, bases, 1, 2);
        System.out.println(showBases(bases));

        Base[] reversed = {new Sub(), new Base(), new Sub()};
        Sub[] narrow = new Sub[3];
        try {
            System.arraycopy(reversed, 0, narrow, 0, 3);
        } catch (ArrayStoreException e) {
            System.out.println(e.getMessage());
        }
        System.out.println(showSubs(narrow));

        Named[] named = new Named[2];
        System.arraycopy(boxes, 0, named, 0, 2);
        System.out.println(showNamed(named));
        Object[] notNamed = {new Box(8), "s"};
        try {
            System.arraycopy(notNamed, 0, named, 0, 2);
        } catch (ArrayStoreException e) {
            System.out.println(e.getMessage());
        }
        System.out.println(showNamed(named));

        Box[] withNull = {null, new Box(9)};
        Plain[] plains = new Plain[2];
        try {
            System.arraycopy(withNull, 0, plains, 0, 2);
        } catch (ArrayStoreException e) {
            System.out.println(e.getMessage());
        }
        System.out.println(showPlains(plains));

        Plain[] source = {null, new Plain()};
        Box[] target = new Box[2];
        try {
            System.arraycopy(source, 0, target, 0, 2);
        } catch (ArrayStoreException e) {
            System.out.println(e.getMessage());
        }
        System.out.println(showBoxes(target));

        Object[] nulls = {null, null};
        Sub[] fromNulls = {new Sub(), new Sub()};
        System.arraycopy(nulls, 0, fromNulls, 0, 2);
        System.out.println(showSubs(fromNulls));

        Box[] holes = new Box[2];
        Plain[] filled = {new Plain()};
        System.arraycopy(holes, 0, filled, 0, 1);
        System.out.println(showPlains(filled));
    }
}
