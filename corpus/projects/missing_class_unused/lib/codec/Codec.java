package codec;

import fast.NativeCodec;

/** A codec that uses the native one only when asked to, and otherwise its own code. */
public final class Codec {
    private final boolean useNative;
    private NativeCodec nativeCodec;

    public Codec(boolean useNative) {
        this.useNative = useNative;
    }

    public static Object best(boolean useNative) {
        if (useNative) {
            return new FastCodec();
        }
        return "plain codec";
    }

    public String encode(String text) {
        if (useNative) {
            if (nativeCodec == null) {
                nativeCodec = new NativeCodec();
            }
            return nativeCodec.encode(text);
        }
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < text.length(); i++) {
            out.append((char) (text.charAt(i) + 1));
        }
        return out.toString();
    }

    public static String name(NativeCodec codec) {
        return codec == null ? "plain" : codec.name();
    }

    public static NativeCodec.Options options() {
        return null;
    }

    public boolean usesNative() {
        return useNative && nativeCodec != null;
    }

    public int level() {
        NativeCodec.Options options = options();
        return options == null ? 1 : options.level;
    }

    public Runnable warmUp() {
        return () -> NativeCodec.warmUp();
    }
}
