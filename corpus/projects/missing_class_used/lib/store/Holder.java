package store;

import engine.Engine;

/** A static initializer that needs the engine: it fails once, and the class is unusable after. */
public class Holder {
    static final int WIDTH = Engine.make().size;
    static int ready = 1;

    public static int width() {
        return WIDTH;
    }
}
