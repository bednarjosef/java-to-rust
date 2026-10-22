// `==` on `Object`s that may both be strings or boxes, which the translation compares at run
// time: different values are different objects, a value is never the same as an object of the
// program, and an object of the program or null is compared by identity.
public class ObjectValueIdentity {
    static boolean same(Object a, Object b) {
        return a == b;
    }

    public static void main(String[] args) {
        Object self = new ObjectValueIdentity();
        Object[] items = {"a", "b", 1, 2, 2L, 'c', true, false, 1.5, self, null};
        for (int i = 0; i < items.length; i++) {
            StringBuilder line = new StringBuilder();
            for (int j = 0; j < items.length; j++) {
                if (i == j && i < items.length - 2) {
                    line.append('-');
                } else {
                    line.append(same(items[i], items[j]) ? 'T' : 'F');
                }
            }
            System.out.println(line);
        }
        System.out.println(items[0] != items[9]);
    }
}
