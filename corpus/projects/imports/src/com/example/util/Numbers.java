package com.example.util;

public class Numbers {
    public static final int LIMIT = 100;

    public static int square(int x) {
        return x * x;
    }

    public static int cube(int x) {
        return x * square(x);
    }

    /** Same name as `Strings.describe`, in the same package. */
    public static String describe() {
        return "numbers up to " + LIMIT;
    }
}
