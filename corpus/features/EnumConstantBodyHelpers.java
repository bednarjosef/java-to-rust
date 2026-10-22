// Methods of enum constants' bodies that override nothing: helpers that only the body's own
// code calls, as javac's Option.INHERIT_RUNTIME_ENVIRONMENT has `getSupportedRuntimeOptions()`.
public class EnumConstantBodyHelpers {
    enum Option {
        VERBOSE("-v"),
        QUIET("-q"),
        ALL("--all") {
            @Override
            String describe(String arg) {
                StringBuilder out = new StringBuilder();
                for (Option o : supported()) {
                    if (matches(o, arg)) {
                        out.append(o.flag).append(' ');
                    }
                }
                return "all of " + count(supported()) + ": " + out.toString().trim();
            }

            private Option[] supported() {
                Option[] options = {VERBOSE, QUIET};
                return options;
            }

            private boolean matches(Option o, String arg) {
                return arg.isEmpty() || o.flag.contains(arg);
            }

            private int count(Option[] options) {
                return options.length + calls++;
            }
        },
        NAMED("--name") {
            @Override
            String describe(String arg) {
                return label(arg) + " (" + label("") + ")";
            }

            private String label(String arg) {
                return arg.isEmpty() ? "unnamed" : "named " + arg.toUpperCase();
            }
        };

        final String flag;
        int calls;

        Option(String flag) {
            this.flag = flag;
        }

        String describe(String arg) {
            return flag + " " + arg;
        }
    }

    public static void main(String[] args) {
        for (Option o : Option.values()) {
            System.out.println(o.describe("") + " | " + o.describe("v"));
        }
        System.out.println(Option.ALL.describe("q"));
        System.out.println(Option.ALL.calls);
    }
}
