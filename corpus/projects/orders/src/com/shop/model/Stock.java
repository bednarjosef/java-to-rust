package com.shop.model;

public class Stock {
    public static class OutOfStock extends Exception {
        public OutOfStock(String item, int wanted, int left) {
            super(item + ": wanted " + wanted + ", " + left + " left");
        }
    }

    private final String item;
    private int count;

    public Stock(String item, int count) {
        this.item = item;
        this.count = count;
    }

    public void take(int n) throws OutOfStock {
        if (n <= 0) {
            throw new IllegalArgumentException("bad count " + n);
        }
        if (n > count) {
            throw new OutOfStock(item, n, count);
        }
        count -= n;
    }

    public int count() {
        return count;
    }
}
