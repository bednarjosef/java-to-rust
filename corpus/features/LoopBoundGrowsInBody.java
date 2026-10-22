// A `for` loop's bound can change while the loop runs: a field written in the body, a field
// written by a method the body calls, a static field, an array or string field whose length
// changes, and a local. Java re-reads the bound on every iteration, so each loop must run as long
// as the bound grows, not as a Rust range that reads the bound once.
public class LoopBoundGrowsInBody {
    static int limit = 3;
    int count = 2;
    int[] items = new int[2];
    String text = "ab";

    void grow() {
        count++;
    }

    int size() {
        return count;
    }

    void growItems() {
        items = new int[items.length + 1];
    }

    void growText() {
        text = text + "c";
    }

    static void growLimit() {
        limit++;
    }

    int fieldWrittenInBody() {
        int seen = 0;
        for (int i = 0; i < count; i++) {
            if (count < 5) count++;
            seen++;
        }
        return seen;
    }

    int fieldWrittenByMethod() {
        int seen = 0;
        for (int i = 0; i < count; i++) {
            if (count < 6) grow();
            seen++;
        }
        return seen;
    }

    int boundIsMethodCall() {
        int seen = 0;
        for (int i = 0; i < size(); i++) {
            if (count < 8) grow();
            seen++;
        }
        return seen;
    }

    int arrayLengthChanges() {
        int seen = 0;
        for (int i = 0; i < items.length; i++) {
            if (items.length < 5) growItems();
            seen++;
        }
        return seen;
    }

    int stringLengthChanges() {
        int seen = 0;
        for (int i = 0; i < text.length(); i++) {
            if (text.length() < 6) growText();
            seen++;
        }
        return seen;
    }

    public static void main(String[] args) {
        LoopBoundGrowsInBody o = new LoopBoundGrowsInBody();
        System.out.println(o.fieldWrittenInBody() + " " + o.count);

        o = new LoopBoundGrowsInBody();
        System.out.println(o.fieldWrittenByMethod() + " " + o.count);

        o = new LoopBoundGrowsInBody();
        System.out.println(o.boundIsMethodCall() + " " + o.count);

        o = new LoopBoundGrowsInBody();
        System.out.println(o.arrayLengthChanges() + " " + o.items.length);

        o = new LoopBoundGrowsInBody();
        System.out.println(o.stringLengthChanges() + " " + o.text);

        int seen = 0;
        for (int i = 0; i < limit; i++) {
            if (limit < 6) growLimit();
            seen++;
        }
        System.out.println(seen + " " + limit);

        int n = 3;
        int local = 0;
        for (int i = 0; i < n; i++) {
            if (n < 6) n++;
            local++;
        }
        System.out.println(local + " " + n);
    }
}
