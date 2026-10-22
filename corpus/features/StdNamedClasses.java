enum Option {
    VERBOSE("-v"), QUIET("-q");

    final String flag;

    Option(String flag) {
        this.flag = flag;
    }
}

class Result {
    final String text;
    final Option option;

    Result(String text, Option option) {
        this.text = text;
        this.option = option;
    }

    @Override
    public String toString() {
        return text + " " + option + " " + option.flag;
    }
}

public class StdNamedClasses {
    static Result find(String flag) {
        for (Option o : Option.values()) {
            if (o.flag.equals(flag)) {
                return new Result("found", o);
            }
        }
        return null;
    }

    public static void main(String[] args) {
        Object[] found = {find("-v"), find("-q"), find("-x"), Option.QUIET};
        for (Object o : found) {
            System.out.println(o);
        }
    }
}
