import java.util.Arrays;

public class ArraysSortStrings {
    public static void main(String[] args) {
        String[] words = { "pear", "apple", "fig" };
        Arrays.sort(words);
        System.out.println(Arrays.toString(words) + " " + words.length);
    }
}
