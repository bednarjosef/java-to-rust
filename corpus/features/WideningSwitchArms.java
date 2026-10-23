// A pattern switch of type `long` whose default arm is an `int` constant, as in
// `FileChannelImpl.transferToDirectInternal`: the arm widens to the switch's type.
public class WideningSwitchArms {
    static final class Status {
        static final int UNSUPPORTED_CASE = -6;
    }

    interface Target {
    }

    static final class FileTarget implements Target {
    }

    static final class SocketTarget implements Target {
    }

    static long toFile(FileTarget f) {
        return 10L;
    }

    static long toSocket(SocketTarget s) {
        return 20L;
    }

    static long transfer(Target target) {
        return switch (target) {
            case FileTarget f -> toFile(f);
            case SocketTarget s -> toSocket(s);
            default -> Status.UNSUPPORTED_CASE;
        };
    }

    static double scaled(Object o) {
        return switch (o) {
            case Integer i -> i * 2;
            case String s -> s.length();
            default -> Status.UNSUPPORTED_CASE;
        };
    }

    public static void main(String[] args) {
        System.out.println(transfer(new FileTarget()));
        System.out.println(transfer(new Target() {}));
        System.out.println(scaled(21) + " " + scaled("abc") + " " + scaled(1.5));
    }
}
