import java.util.Arrays;
import java.util.LinkedHashSet;
import java.util.Set;
import java.util.TreeMap;

// An enum whose constructor splits its text into names and builds a set of its choices, as
// javac's `Option` does: the constructors run when the enum is initialized.
public enum EnumConstructorsBuildCollections {
    HELP("--help -help -?", "opt.help"),
    RELEASE("--release", "opt.release", "8", "11", "17", "21"),
    ENCODING("-encoding --encoding", "opt.encoding", "UTF-8", "US-ASCII");

    final String[] names;
    final String primaryName;
    final String descrKey;
    final Set<String> choices;

    EnumConstructorsBuildCollections(String text, String descrKey, String... choices) {
        this.names = text.trim().split("\\s+");
        if (names.length < 1) {
            throw new AssertionError(text);
        }
        this.primaryName = names[0];
        this.descrKey = descrKey;
        this.choices = choices.length == 0 ? null : new LinkedHashSet<>(Arrays.asList(choices));
    }

    boolean matches(String option) {
        for (String name : names) {
            if (name.equals(option)) {
                return true;
            }
        }
        return false;
    }

    public static void main(String[] args) {
        TreeMap<String, EnumConstructorsBuildCollections> byName = new TreeMap<>();
        for (EnumConstructorsBuildCollections o : values()) {
            for (String name : o.names) {
                byName.put(name, o);
            }
            System.out.println(o.primaryName + " " + o.names.length + " " + o.descrKey + " " + o.choices);
        }
        System.out.println(byName.keySet());
        System.out.println(ENCODING.matches("--encoding") + " " + HELP.matches("--encoding"));
    }
}
