import java.lang.reflect.Field;
import sun.misc.Unsafe;

// Field offsets that a static helper finds for its callers, as JCTools' queues in netty find
// theirs: `P_INDEX_OFFSET = UnsafeAccess.fieldOffset(MpscArrayQueueProducerIndexField.class,
// "producerIndex")`, with `UNSAFE.objectFieldOffset(clz.getDeclaredField(fieldName))` in the
// helper, whose class `sun.misc.Unsafe`'s warning names.
public class UnsafeOffsetHelpers {
    static final class UnsafeAccess {
        static final Unsafe UNSAFE;

        static {
            try {
                Field field = Unsafe.class.getDeclaredField("theUnsafe");
                field.setAccessible(true);
                UNSAFE = (Unsafe) field.get(null);
            } catch (Exception e) {
                throw new RuntimeException(e);
            }
        }

        static long fieldOffset(Class<?> clz, String fieldName) throws RuntimeException {
            try {
                return UNSAFE.objectFieldOffset(clz.getDeclaredField(fieldName));
            } catch (NoSuchFieldException e) {
                throw new RuntimeException(e);
            }
        }
    }

    static class ProducerIndex {
        private static final long P_INDEX_OFFSET = UnsafeAccess.fieldOffset(ProducerIndex.class, "producerIndex");
        private volatile long producerIndex;

        long lvProducerIndex() {
            return producerIndex;
        }

        boolean casProducerIndex(long expect, long newValue) {
            return UnsafeAccess.UNSAFE.compareAndSwapLong(this, P_INDEX_OFFSET, expect, newValue);
        }
    }

    static class Queue extends ProducerIndex {
        private static final long C_INDEX_OFFSET = UnsafeAccess.fieldOffset(Queue.class, "consumerIndex");
        private volatile long consumerIndex;

        void soConsumerIndex(long newValue) {
            UnsafeAccess.UNSAFE.putOrderedLong(this, C_INDEX_OFFSET, newValue);
        }
    }

    public static void main(String[] args) {
        Queue q = new Queue();
        System.out.println(q.casProducerIndex(0, 5) + " " + q.casProducerIndex(0, 7) + " " + q.lvProducerIndex());
        q.soConsumerIndex(q.consumerIndex + 1);
        q.soConsumerIndex(q.consumerIndex + 1);
        System.out.println(q.consumerIndex);
    }
}
