package fast;

/** A native codec that the library can use when it is installed; the program runs without it. */
public class NativeCodec {
    public static class Options {
        public int level = 9;
    }

    public static void warmUp() {
        System.out.println("warming up");
    }

    public String encode(String text) {
        return "native " + text;
    }

    public String name() {
        return "native";
    }
}
