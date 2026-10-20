package com.shop;

import com.shop.model.Item;
import com.shop.model.Registry;

import static com.shop.model.Registry.total;

/** Static fields and constructors used across packages, and two classes named `Item`. */
public class Main {
    public static void main(String[] args) {
        Item apple = new Item("apple", 3);
        Item pear = new Item("pear");
        System.out.println(Registry.created);
        Registry.created += 10;
        System.out.println(Registry.created);
        System.out.println(apple.describe() + " " + pear.describe());
        com.shop.legacy.Item old = new com.shop.legacy.Item(7);
        System.out.println(old.code());
        System.out.println(com.shop.legacy.Item.PREFIX + Registry.NAME);
        Registry.record(5);
        Registry.record(7);
        System.out.println(total());
        System.out.println(Registry.label);
        Registry.label = "changed";
        System.out.println(Registry.label);
        Counter.bump();
        Counter.bump();
        System.out.println(Counter.hits);
        Registry.Entry entry = new Registry.Entry(4);
        System.out.println(entry.value + Registry.created);
    }
}
