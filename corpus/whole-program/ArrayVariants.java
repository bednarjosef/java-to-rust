// Arrays of classes that share a simple name, held as `Object`s, whose variants need names of
// their own: the JDK's `ArrayCacheByte.Bucket[]` and `ArrayCacheInt.Bucket[]`.
public class ArrayVariants {
    static class Bytes {
        static class Bucket {
            int size = 1;
        }
    }

    static class Ints {
        static class Bucket {
            int size = 2;
        }
    }

    static String describe(Object o) {
        if (o instanceof Bytes.Bucket[] b) {
            return "byte buckets " + b.length + " " + b[0].size;
        }
        if (o instanceof Ints.Bucket[] b) {
            return "int buckets " + b.length + " " + b[0].size;
        }
        if (o instanceof Ints.Bucket[][] b) {
            return "int bucket rows " + b.length;
        }
        return "other";
    }

    public static void main(String[] args) {
        Object[] all = {new Bytes.Bucket[] {new Bytes.Bucket()}, new Ints.Bucket[] {new Ints.Bucket(), new Ints.Bucket()}, new Ints.Bucket[3][1], "x"};
        for (Object o : all) {
            System.out.println(describe(o));
        }
    }
}
