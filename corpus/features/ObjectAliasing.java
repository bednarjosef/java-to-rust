class Box {
    int value;

    Box(int value) {
        this.value = value;
    }

    void add(int n) {
        value += n;
    }

    int get() {
        return value;
    }
}

public class ObjectAliasing {
    static void twice(Box b) {
        b.add(1);
        b.add(1);
    }

    public static void main(String[] args) {
        Box a = new Box(1);
        Box b = a;
        b.add(5);
        System.out.println(a.get());
        twice(a);
        System.out.println(b.get());
        a.add(10);
        System.out.println(a.value + b.get());
        Box c = new Box(0);
        Box d = c;
        c.value = 3;
        System.out.println(d.value);
        d.add(c.get());
        System.out.println(c.value);
    }
}
