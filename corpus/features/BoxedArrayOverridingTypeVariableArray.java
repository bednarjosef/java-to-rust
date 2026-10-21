// A method that takes an `Integer[]` and overrides one that takes the `T[]` of `Sink<Integer>`,
// as `Node.OfInt.copyInto` does: the bridge that erasure needs passes it the same array, which it
// fills, so a call through the generic method and a direct call both fill the caller's array.
public class BoxedArrayOverridingTypeVariableArray {
    interface Sink<T> {
        void copyInto(T[] array, int offset);
    }

    interface IntSink extends Sink<Integer> {
        int[] values();

        @Override
        default void copyInto(Integer[] boxed, int offset) {
            int[] v = values();
            for (int i = 0; i < v.length; i++) {
                boxed[offset + i] = v[i];
            }
        }
    }

    static final class Three implements IntSink {
        public int[] values() {
            return new int[] {4, 5, 6};
        }
    }

    static <T> void fill(Sink<T> sink, T[] into) {
        sink.copyInto(into, 1);
    }

    public static void main(String[] args) {
        Integer[] direct = {0, 0, 0, 0};
        Three three = new Three();
        three.copyInto(direct, 0);
        System.out.println(direct[0] + direct[1] + direct[2] + direct[3]);
        Integer[] viaGeneric = {1, 1, 1, 1};
        fill(three, viaGeneric);
        int sum = 0;
        for (Integer x : viaGeneric) {
            sum += x;
        }
        System.out.println(sum + " " + viaGeneric[1] + " " + viaGeneric[3]);
    }
}
