// The value of a store in an array that is an `Object[]` where arrays meet a type variable's (see
// covariant arrays), used as its own class, as javac's Code does with its stack map frames.
public class CovariantArrayStoreValues {
    static class Frame {
        int pc;
    }

    static Object[] kept(Object[] array, int size) {
        return size < array.length ? array : null;
    }

    Frame[] buffer = new Frame[4];
    int size;

    Frame push(int pc) {
        buffer = (Frame[]) kept(buffer, size);
        Frame frame = buffer[size++] = new Frame();
        frame.pc = pc;
        buffer[size - 1] = frame;
        return frame;
    }

    public static void main(String[] args) {
        CovariantArrayStoreValues code = new CovariantArrayStoreValues();
        code.push(3);
        Object[] mixed = {new Frame(), "frame"};
        System.out.println(mixed[1]);
        System.out.println(code.push(7).pc + " " + code.buffer[0].pc + " " + code.size);
    }
}
