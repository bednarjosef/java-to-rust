// `System.getenv(name)`, which reads the environment the program runs in, as javac reads
// `JDK_JAVAC_OPTIONS`. The output names only what is the same from run to run: a variable that is
// certainly unset, names that no variable can have, and that `PATH` is set, the same each time it
// is read.
public class EnvironmentVariables {
    static String read(String name) {
        return System.getenv(name);
    }

    public static void main(String[] args) {
        System.out.println(System.getenv("JAVA_TO_RUST_CERTAINLY_UNSET_VARIABLE"));
        for (String name : new String[] {"", "A=B", "=PATH", "JAVA_TO_RUST_CERTAINLY_UNSET_VARIABLE"}) {
            String value = read(name);
            System.out.println("[" + name + "] " + (value == null ? "unset" : "set"));
        }
        String path = System.getenv("PATH");
        System.out.println("PATH set: " + (path != null) + ", same again: " + path.equals(read("PATH")));
        String options = System.getenv("JAVA_TO_RUST_CERTAINLY_UNSET_VARIABLE");
        System.out.println(options == null ? "no options" : "options: " + options.length());
    }
}
