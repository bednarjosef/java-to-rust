// Instance initializer blocks run after the field initializers and before the rest of every
// constructor, once per object: a constructor that calls another with this(...) does not run them
// again. A class without a constructor runs them in its default constructor.
public class InitializerBlocks {
    static int created;

    static class Table {
        int size = 4;
        int[] squares = new int[size];
        String label;

        {
            for (int i = 0; i < size; i++) {
                squares[i] = i * i;
            }
            created++;
            System.out.println("block runs, size " + size);
        }

        {
            label = "table#" + created;
        }

        Table() {
            System.out.println("Table() after the blocks: " + label);
        }

        Table(int extra) {
            this();
            size += extra;
            System.out.println("Table(int) after this(): size " + size);
        }

        Table(String name) {
            label = name;
            System.out.println("Table(String): " + label + " " + squares[3]);
        }
    }

    static class Counter {
        int count;

        {
            count = 10;
        }

        int next() {
            return ++count;
        }
    }

    public static void main(String[] args) {
        Table a = new Table();
        Table b = new Table(3);
        Table c = new Table("named");
        System.out.println(a.label + " " + b.label + " " + b.size + " " + c.label + " " + created);
        Counter counter = new Counter();
        counter.next();
        System.out.println(counter.next());
    }
}
