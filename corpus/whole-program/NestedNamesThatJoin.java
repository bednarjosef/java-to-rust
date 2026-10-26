// Two nested classes whose names are the same once the dots go: `Table.ColumnKeySet` and
// `Table.Column.KeySet`, as in Guava's `StandardTable`, each with statics, a static initializer,
// a synchronized static method and objects.
public class NestedNamesThatJoin {
    static class Table {
        static class ColumnKeySet {
            static int made;
            static final String NAME;

            static {
                NAME = "column key set";
            }

            final int size;

            ColumnKeySet(int size) {
                this.size = size;
                made++;
            }

            static synchronized String describe() {
                return NAME + " x" + made;
            }
        }

        static class Column {
            static class KeySet {
                static int made;
                static final String NAME;

                static {
                    NAME = "key set of a column";
                }

                final String key;

                KeySet(String key) {
                    this.key = key;
                    made++;
                }

                static synchronized String describe() {
                    return NAME + " x" + made;
                }
            }
        }
    }

    public static void main(String[] args) {
        Table.ColumnKeySet a = new Table.ColumnKeySet(3);
        Table.Column.KeySet b = new Table.Column.KeySet("k");
        Table.Column.KeySet c = new Table.Column.KeySet("l");
        System.out.println(a.size + " " + b.key + c.key);
        System.out.println(Table.ColumnKeySet.describe());
        System.out.println(Table.Column.KeySet.describe());
        Object o = b;
        System.out.println(o.getClass().getName() + " " + a.getClass().getSimpleName());
        System.out.println(o instanceof Table.ColumnKeySet);
    }
}
