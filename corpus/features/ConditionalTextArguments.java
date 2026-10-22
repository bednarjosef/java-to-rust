// A conditional of strings passed where a `CharSequence` is expected, which javac types as the
// parameter: the text calls take it as a string, as `ExplodedImage.slashesToDots` does.
public class ConditionalTextArguments {
    private final String separator;

    ConditionalTextArguments(String separator) {
        this.separator = separator;
    }

    String dots(String path) {
        return path.replace(separator != null ? separator : "/", ".");
    }

    public static void main(String[] args) {
        System.out.println(new ConditionalTextArguments(null).dots("java/lang/Object"));
        System.out.println(new ConditionalTextArguments("\\").dots("java\\util\\List"));
        String word = args.length > 0 ? args[0] : "list";
        System.out.println("a list of lists".contains(args.length > 5 ? "map" : word));
        System.out.println("a list of lists".replace(word.isEmpty() ? "x" : word, word.length() > 3 ? "set" : "map"));
    }
}
