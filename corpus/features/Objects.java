class Counter {
    int count;
    int step = 2;

    Counter(int start) {
        count = start;
    }

    Counter() {
        this(10);
    }

    void tick() {
        count += step;
        count++;
    }

    int value() {
        return count;
    }
}

public class Objects {
    public static void main(String[] args) {
        Counter a = new Counter(5);
        Counter b = new Counter();
        a.tick();
        b.tick();
        b.tick();
        System.out.println(a.value());
        System.out.println(b.value());
        System.out.println(a.count + b.count);
        a.step = 7;
        a.tick();
        System.out.println(a.count);
        b.count = -1;
        System.out.println(b.value());
    }
}
