package codec;

import fast.NativeCodec;

/** A codec on the native one: it cannot be loaded without it, and the program never loads it. */
public class FastCodec extends NativeCodec {
    @Override
    public String name() {
        return "fast " + super.name();
    }
}
