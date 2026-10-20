package com.shop.model;

public class Registry {
    public static int created;
    public static final String NAME = "registry";
    public static String label = "start";
    private static int sum;

    public static void record(int amount) {
        sum += amount;
    }

    public static int total() {
        return sum;
    }

    public static class Entry {
        public int value;

        public Entry(int value) {
            this.value = value * created;
        }
    }
}
