public class StringValueOfChars {
    public static void main(String[] args) {
        char[] word = {'h', 'e', 'l', 'l', 'o', ' ', 'é', '世'};
        String all = String.valueOf(word);
        String middle = String.valueOf(word, 1, 3);
        System.out.println(all + "|" + middle + "|" + String.valueOf(word, 6, 0) + "|");
        System.out.println(String.copyValueOf(word) + " " + String.copyValueOf(word, 6, 2));
        System.out.println(all.equals(new String(word)) + " " + all.length());
        // The array is copied: changing it afterwards does not change the string.
        word[0] = 'J';
        System.out.println(all + " " + String.valueOf(word, 0, 2));
        char[] none = new char[0];
        System.out.println("[" + String.valueOf(none) + "]");
        try {
            String.valueOf(word, 5, 9);
        } catch (StringIndexOutOfBoundsException e) {
            System.out.println("out of range");
        }
        System.out.println(String.valueOf(word, 7, 1));
    }
}
