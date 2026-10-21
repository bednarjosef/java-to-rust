// Enum fields that start as null, as `java.io.File`'s lazily checked `status` does: one with no
// initializer, one initialized to null, read into a local, tested, set and printed.
public class EnumFieldsStartingNull {
    enum Status { INVALID, CHECKED }

    enum Mode { READ, WRITE }

    static class Path {
        private final String path;
        private Status status = null;
        private Mode mode;

        Path(String path) {
            this.path = path;
        }

        boolean isInvalid() {
            Status s = status;
            if (s == null) {
                s = path.indexOf('\u0000') >= 0 ? Status.INVALID : Status.CHECKED;
                status = s;
            }
            return s == Status.INVALID;
        }

        void open(boolean write) {
            if (mode == null) {
                mode = write ? Mode.WRITE : Mode.READ;
            }
        }
    }

    public static void main(String[] args) {
        Path a = new Path("a.txt");
        System.out.println(a.status + " " + a.mode);
        System.out.println(a.isInvalid() + " " + a.status);
        System.out.println(a.isInvalid());
        Path b = new Path("b\u0000");
        System.out.println(b.isInvalid() + " " + b.status);
        a.open(true);
        a.open(false);
        System.out.println(a.mode + " " + (a.mode == Mode.WRITE) + " " + (b.mode != null));
        Status copy = b.status;
        System.out.println(copy == Status.INVALID);
    }
}
