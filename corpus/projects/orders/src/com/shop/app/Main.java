package com.shop.app;

import com.shop.model.Stock;

public class Main {
    static String order(Stock s, int n) {
        try {
            s.take(n);
            return "took " + n + ", " + s.count() + " left";
        } catch (Stock.OutOfStock e) {
            return "failed: " + e.getMessage() + " (" + e.getClass().getName() + ")";
        } catch (IllegalArgumentException e) {
            return "rejected: " + e;
        }
    }

    public static void main(String[] args) {
        Stock apples = new Stock("apples", 5);
        System.out.println(order(apples, 3));
        System.out.println(order(apples, 3));
        System.out.println(order(apples, 0));
        System.out.println(order(apples, 2));
    }
}
