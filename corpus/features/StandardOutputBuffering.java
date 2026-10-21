// System.out's own buffer: `write(int)` holds a byte until a newline, a print, a flush or a full
// buffer of 128 bytes passes it on, and what it still holds when the program ends is lost.
public class StandardOutputBuffering {
    public static void main(String[] args) {
        System.out.write('a');
        System.out.write('b');
        System.out.println(" after two held bytes");
        System.out.write('c');
        System.out.write('\n');
        System.out.write('d');
        System.out.flush();
        System.out.write('\n');
        System.out.write('e');
        System.out.print("");
        System.out.write('f');
        System.out.print("g");
        System.out.println();
        // 130 bytes without a newline: the buffer passes on the first 128 when it is full.
        for (int i = 0; i < 130; i++) {
            System.out.write('0' + i % 10);
        }
        // The low eight bits of the int are the byte; 266 is a newline but does not flush.
        System.out.write(266);
        System.err.println("standard error does not flush standard output's buffer");
        System.out.write('z');
        System.out.write('z');
    }
}
