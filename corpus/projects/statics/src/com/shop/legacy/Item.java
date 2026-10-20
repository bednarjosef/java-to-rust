package com.shop.legacy;

import com.shop.model.Registry;

/** Same simple name as `com.shop.model.Item`. */
public class Item {
    public static final String PREFIX = "legacy-";
    private int id;

    public Item(int id) {
        this.id = id * 100 + Registry.created;
    }

    public int code() {
        return id;
    }
}
