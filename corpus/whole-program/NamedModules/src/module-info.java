// A program that is a named module: what its classes' `Module` answers, as the JVM's boot layer.
module app {
    exports app.api;
    opens app.data to java.base;
    uses app.api.Greeter;
    provides app.api.Greeter with app.impl.Hello;
}
