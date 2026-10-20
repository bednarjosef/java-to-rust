package com.example.util;

public class Strings {
    public static String repeat(String s, int times) {
        String out = "";
        for (int i = 0; i < times; i++) {
            out += s;
        }
        return out;
    }

    public static String reverse(String s) {
        String out = "";
        for (int i = s.length() - 1; i >= 0; i--) {
            out += s.charAt(i);
        }
        return out;
    }

    public static String describe() {
        return "strings of length " + Numbers.square(2);
    }
}
