package app.impl;

import app.api.Greeter;

public class Hello implements Greeter {
    public String greet(String name) {
        return "hello, " + name;
    }
}
