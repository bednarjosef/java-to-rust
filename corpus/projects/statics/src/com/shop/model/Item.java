package com.shop.model;

public class Item {
    private String name = "";
    private int quantity;

    public Item(String name, int quantity) {
        this.name = name;
        this.quantity = quantity;
        Registry.created++;
    }

    public Item(String name) {
        this(name, 1);
    }

    public String describe() {
        return name + " x" + quantity;
    }
}
