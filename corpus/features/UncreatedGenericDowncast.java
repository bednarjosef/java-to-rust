// Generic classes of which no object is created, read back from an erased container as
// elements of their type: the downcast from Object can only pass null, and the classes have no
// variant in Object's enum to test.
public class UncreatedGenericDowncast {
    static class Item {
        int size;
    }

    static class Cell<T> {
        T value;
    }

    static class Box<T> {
        T value;
    }

    static class Stack<T> {
        T top;
    }

    public static void main(String[] args) {
        Stack<Cell<Item>> cells = new Stack<>();
        System.out.println(cells.equals(new Item()));
        System.out.println(cells.top == null);
        Stack<Box<Item>> boxes = new Stack<>();
        Object o = boxes.top;
        if (o instanceof Box<?> found) {
            System.out.println("box " + (found.value == null));
        } else {
            System.out.println("no box");
        }
    }
}
