package app;

import codec.Codec;

/**
 * A program whose library refers to a class that is not on its class path: the code that uses it
 * never runs, so the program runs as it would with the class.
 */
public class Main {
    public static void main(String[] args) {
        boolean useNative = args.length > 0;
        Codec codec = new Codec(useNative);
        System.out.println(codec.encode("HAL"));
        System.out.println(Codec.name(null));
        System.out.println(Codec.best(useNative));
        System.out.println("uses native: " + codec.usesNative());
        System.out.println("level " + codec.level());
        Runnable warmUp = codec.warmUp();
        System.out.println("made a warm-up: " + (warmUp != null));
    }
}
