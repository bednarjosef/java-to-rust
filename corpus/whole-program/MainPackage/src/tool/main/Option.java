package tool.main;

public enum Option {
    VERBOSE("-verbose"),
    NOWARN("-nowarn");

    private final String flag;

    Option(String flag) {
        this.flag = flag;
    }

    public String flag() {
        return flag;
    }
}
