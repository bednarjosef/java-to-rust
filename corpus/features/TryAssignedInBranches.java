// Locals declared without a value and assigned in both branches of an `if` in a `try` block, read
// after it, as the Minecraft server's `PalettedContainer.unpack` makes a chunk section's storage.
public class TryAssignedInBranches {
    static final class Storage {
        final int bits;

        Storage(int bits) {
            if (bits > 32) {
                throw new IllegalStateException("too many bits: " + bits);
            }
            this.bits = bits;
        }
    }

    static String unpack(int bits, boolean repack) {
        Storage storage;
        String palette;
        try {
            if (!repack) {
                palette = "linear";
                storage = new Storage(bits);
            } else {
                Storage old = new Storage(bits);
                palette = "hashmap";
                storage = new Storage(old.bits * 2);
            }
        } catch (IllegalStateException e) {
            return "error: " + e.getMessage();
        }
        return palette + " " + storage.bits;
    }

    public static void main(String[] args) {
        System.out.println(unpack(4, false));
        System.out.println(unpack(5, true));
        System.out.println(unpack(20, true));
        System.out.println(unpack(40, false));
    }
}
