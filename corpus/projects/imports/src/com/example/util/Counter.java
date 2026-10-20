package com.example.util;

public class Counter {
    private String name = "";
    private int total;

    public Counter(String name) {
        this.name = name;
    }

    public void add(int amount) {
        total += amount;
    }

    public int total() {
        return total;
    }

    @Override
    public String toString() {
        return name + "=" + total;
    }
}
